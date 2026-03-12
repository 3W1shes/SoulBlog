use crate::{
    error::{AppError, Result},
    models::domain::*,
    services::Database,
};
use chrono::{Duration, Utc};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use reqwest::Client;
use trust_dns_resolver::{
    config::{ResolverConfig, ResolverOpts},
    TokioAsyncResolver,
};

/// Configuration for domain service
#[derive(Clone)]
pub struct DomainConfig {
    /// Base domain for subdomains (e.g., "platform.com")
    pub base_domain: String,
    /// DNS verification timeout in seconds
    pub dns_verification_timeout: u64,
    /// SSL certificate provider API endpoint
    pub ssl_provider_endpoint: Option<String>,
    /// SSL certificate provider API key
    pub ssl_provider_api_key: Option<String>,
    /// Whether to auto-provision SSL certificates
    pub auto_provision_ssl: bool,
    /// Webhook URL for SSL certificate events
    pub ssl_webhook_url: Option<String>,
}

#[derive(Clone)]
pub struct DomainService {
    db: Arc<Database>,
    config: DomainConfig,
    http_client: Client,
    dns_resolver: TokioAsyncResolver,
}

impl DomainService {
    pub async fn new(db: Arc<Database>, config: DomainConfig) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        let dns_resolver = TokioAsyncResolver::tokio(
            ResolverConfig::default(),
            ResolverOpts::default(),
        );

        Ok(Self {
            db,
            config,
            http_client,
            dns_resolver,
        })
    }

    /// Create a subdomain for a publication
    pub async fn create_subdomain(
        &self,
        publication_id: &str,
        request: CreateSubdomainRequest,
    ) -> Result<DomainResponse> {
        debug!("Creating subdomain {} for publication {}", request.subdomain, publication_id);

        // Validate the subdomain request
        request.validate()
            .map_err(|errors| AppError::Validation(errors.join(", ")))?;

        // Check if subdomain is available
        self.check_subdomain_availability(&request.subdomain).await?;

        // Generate full subdomain
        let full_subdomain = format!("{}.{}", request.subdomain, self.config.base_domain);

        // Create domain record
        let domain = PublicationDomain {
            id: Uuid::new_v4(),
            publication_id: Uuid::parse_str(publication_id)
                .map_err(|_| AppError::Validation("Invalid publication ID".to_string()))?,
            domain_type: DomainType::Subdomain,
            subdomain: Some(full_subdomain.clone()),
            custom_domain: None,
            status: DomainStatus::Active, // Subdomains are active immediately
            verification_token: None,
            verified_at: Some(Utc::now()),
            ssl_status: if self.config.auto_provision_ssl { 
                SSLStatus::Pending 
            } else { 
                SSLStatus::None 
            },
            ssl_expires_at: None,
            is_primary: request.is_primary.unwrap_or(false),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // If this is marked as primary, update other domains
        if domain.is_primary {
            self.update_primary_domain(publication_id, &domain.id).await?;
        }

        // Save to database
        let created_domain: PublicationDomain = self.db.create("publication_domain", domain).await?;

        // Auto-provision SSL if enabled
        if self.config.auto_provision_ssl {
            self.provision_ssl_certificate(&created_domain.id.to_string()).await?;
        }

        info!("Created subdomain {} for publication {}", full_subdomain, publication_id);

        Ok(DomainResponse {
            domain: created_domain,
            verification_records: None,
        })
    }

    /// Add a custom domain to a publication
    pub async fn add_custom_domain(
        &self,
        publication_id: &str,
        request: AddCustomDomainRequest,
    ) -> Result<DomainResponse> {
        debug!("Adding custom domain {} for publication {}", request.domain, publication_id);

        // Validate the domain request
        request.validate()
            .map_err(|errors| AppError::Validation(errors.join(", ")))?;

        // Check if domain is already in use
        self.check_custom_domain_availability(&request.domain).await?;

        // Generate verification token
        let verification_token = self.generate_verification_token();

        // Create domain record
        let domain = PublicationDomain {
            id: Uuid::new_v4(),
            publication_id: Uuid::parse_str(publication_id)
                .map_err(|_| AppError::Validation("Invalid publication ID".to_string()))?,
            domain_type: DomainType::Custom,
            subdomain: None,
            custom_domain: Some(request.domain.clone()),
            status: DomainStatus::Pending,
            verification_token: Some(verification_token.clone()),
            verified_at: None,
            ssl_status: SSLStatus::None,
            ssl_expires_at: None,
            is_primary: request.is_primary.unwrap_or(false),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Save to database
        let created_domain: PublicationDomain = self.db.create("publication_domain", domain).await?;

        // Create verification records
        let verification_records = self.create_verification_records(&created_domain).await?;

        info!("Added custom domain {} for publication {}", request.domain, publication_id);

        Ok(DomainResponse {
            domain: created_domain,
            verification_records: Some(verification_records),
        })
    }

    /// Verify a custom domain
    pub async fn verify_domain(
        &self,
        domain_id: &str,
    ) -> Result<DomainVerificationResponse> {
        debug!("Verifying domain {}", domain_id);

        // Get domain record
        let domain: PublicationDomain = self.db
            .get_by_id("publication_domain", domain_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Domain not found".to_string()))?;

        if domain.domain_type != DomainType::Custom {
            return Err(AppError::BadRequest("Only custom domains need verification".to_string()));
        }

        // Get verification records
        let verification_records = self.get_verification_records(domain_id).await?;

        // Perform DNS verification
        let mut all_verified = true;
        let mut errors = Vec::new();
        let mut updated_records = Vec::new();

        for mut record in verification_records {
            match self.verify_dns_record(&domain, &record).await {
                Ok(verified) => {
                    record.is_verified = verified;
                    record.last_checked_at = Some(Utc::now());
                    if !verified {
                        all_verified = false;
                        errors.push(format!("DNS record {} not found or incorrect", record.record_name));
                    }
                }
                Err(e) => {
                    all_verified = false;
                    errors.push(format!("Failed to verify {}: {}", record.record_name, e));
                }
            }
            
            // Update verification record
            let thing = soulcore::prelude::Thing::new(
                "domain_verification_record",
                record.id.to_string(),
            );
            self.db.update(thing, record.clone()).await?;
            updated_records.push(record);
        }

        // Update domain status
        let new_status = if all_verified {
            DomainStatus::Active
        } else {
            DomainStatus::Verifying
        };

        let updates = json!({
            "status": new_status,
            "verified_at": if all_verified { Some(Utc::now()) } else { None },
            "updated_at": Utc::now(),
        });

        self.db.update_by_id_with_json::<PublicationDomain>(
            "publication_domain",
            domain_id,
            updates,
        ).await?;

        // Auto-provision SSL if domain is verified and SSL is enabled
        if all_verified && self.config.auto_provision_ssl {
            self.provision_ssl_certificate(domain_id).await?;
        }

        Ok(DomainVerificationResponse {
            domain_id: domain.id,
            status: new_status,
            verification_records: updated_records,
            verified: all_verified,
            errors: if errors.is_empty() { None } else { Some(errors) },
        })
    }

    /// Get all domains for a publication
    pub async fn get_publication_domains(
        &self,
        publication_id: &str,
    ) -> Result<DomainListResponse> {
        debug!("Getting domains for publication {}", publication_id);

        let query = format!(
            "SELECT * FROM publication_domain WHERE publication_id = '{}' ORDER BY is_primary DESC, created_at DESC",
            publication_id
        );

        let mut response = self.db.query(&query).await?;
        let domains: Vec<PublicationDomain> = response.take(0)?;
        let total = domains.len() as i64;

        Ok(DomainListResponse {
            domains,
            total,
        })
    }

    /// Get domain by ID
    pub async fn get_domain(
        &self,
        domain_id: &str,
    ) -> Result<Option<DomainResponse>> {
        debug!("Getting domain {}", domain_id);

        let domain: Option<PublicationDomain> = self.db
            .get_by_id("publication_domain", domain_id)
            .await?;

        match domain {
            Some(domain) => {
                let verification_records = if domain.domain_type == DomainType::Custom {
                    Some(self.get_verification_records(domain_id).await?)
                } else {
                    None
                };

                Ok(Some(DomainResponse {
                    domain,
                    verification_records,
                }))
            }
            None => Ok(None),
        }
    }

    /// Update domain settings
    pub async fn update_domain(
        &self,
        domain_id: &str,
        request: UpdateDomainRequest,
    ) -> Result<PublicationDomain> {
        debug!("Updating domain {}", domain_id);

        let domain: PublicationDomain = self.db
            .get_by_id("publication_domain", domain_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Domain not found".to_string()))?;

        let mut updates = json!({
            "updated_at": Utc::now(),
        });

        if let Some(is_primary) = request.is_primary {
            updates["is_primary"] = json!(is_primary);
            if is_primary {
                self.update_primary_domain(&domain.publication_id.to_string(), &domain.id).await?;
            }
        }

        self.db
            .update_by_id_with_json("publication_domain", domain_id, updates)
            .await?
            .ok_or_else(|| AppError::Internal("Failed to update domain".to_string()))
    }

    /// Delete a domain
    pub async fn delete_domain(
        &self,
        domain_id: &str,
    ) -> Result<()> {
        debug!("Deleting domain {}", domain_id);

        let domain: PublicationDomain = self.db
            .get_by_id("publication_domain", domain_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Domain not found".to_string()))?;

        // Delete verification records if custom domain
        if domain.domain_type == DomainType::Custom {
            let query = format!(
                "DELETE domain_verification_record WHERE domain_id = '{}'",
                domain_id
            );
            self.db.query(&query).await?;
        }

        // Delete the domain
        self.db.delete_by_id("publication_domain", domain_id).await?;

        info!("Deleted domain {}", domain_id);
        Ok(())
    }

    /// Find publication by domain
    pub async fn find_publication_by_domain(
        &self,
        domain: &str,
    ) -> Result<Option<String>> {
        debug!("Finding publication for domain {}", domain);

        // First check subdomains
        let subdomain_query = format!(
            "SELECT publication_id FROM publication_domain WHERE subdomain = '{}' AND status = 'active' LIMIT 1",
            domain
        );
        
        let mut response = self.db.query(&subdomain_query).await?;
        let results: Vec<serde_json::Value> = response.take(0)?;
        
        if let Some(result) = results.first() {
            if let Some(pub_id) = result.get("publication_id").and_then(|v| v.as_str()) {
                return Ok(Some(pub_id.to_string()));
            }
        }

        // Then check custom domains
        let custom_query = format!(
            "SELECT publication_id FROM publication_domain WHERE custom_domain = '{}' AND status = 'active' LIMIT 1",
            domain
        );
        
        let mut response = self.db.query(&custom_query).await?;
        let results: Vec<serde_json::Value> = response.take(0)?;
        
        if let Some(result) = results.first() {
            if let Some(pub_id) = result.get("publication_id").and_then(|v| v.as_str()) {
                return Ok(Some(pub_id.to_string()));
            }
        }

        Ok(None)
    }

    /// Check subdomain availability
    async fn check_subdomain_availability(&self, subdomain: &str) -> Result<()> {
        let full_subdomain = format!("{}.{}", subdomain, self.config.base_domain);
        
        let existing: Option<PublicationDomain> = self.db
            .find_one("publication_domain", "subdomain", &full_subdomain)
            .await?;

        if existing.is_some() {
            return Err(AppError::Conflict(format!("Subdomain {} is already taken", subdomain)));
        }

        Ok(())
    }

    /// Check custom domain availability
    async fn check_custom_domain_availability(&self, domain: &str) -> Result<()> {
        let existing: Option<PublicationDomain> = self.db
            .find_one("publication_domain", "custom_domain", domain)
            .await?;

        if existing.is_some() {
            return Err(AppError::Conflict(format!("Domain {} is already in use", domain)));
        }

        Ok(())
    }

    /// Generate verification token
    fn generate_verification_token(&self) -> String {
        format!("rainbow-verify-{}", Uuid::new_v4().to_string().replace("-", ""))
    }

    /// Create verification records for a domain
    async fn create_verification_records(
        &self,
        domain: &PublicationDomain,
    ) -> Result<Vec<DomainVerificationRecord>> {
        let custom_domain = domain.custom_domain.as_ref()
            .ok_or_else(|| AppError::Internal("Custom domain not set".to_string()))?;

        let verification_token = domain.verification_token.as_ref()
            .ok_or_else(|| AppError::Internal("Verification token not set".to_string()))?;

        // Create TXT record for domain ownership verification
        let txt_record = DomainVerificationRecord {
            id: Uuid::new_v4(),
            domain_id: domain.id,
            record_type: "TXT".to_string(),
            record_name: format!("_rainbow-verify.{}", custom_domain),
            record_value: verification_token.clone(),
            is_verified: false,
            last_checked_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Create CNAME record for domain routing
        let cname_record = DomainVerificationRecord {
            id: Uuid::new_v4(),
            domain_id: domain.id,
            record_type: "CNAME".to_string(),
            record_name: custom_domain.clone(),
            record_value: format!("domains.{}", self.config.base_domain),
            is_verified: false,
            last_checked_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Save records to database
        let txt_record: DomainVerificationRecord = self.db.create("domain_verification_record", txt_record).await?;
        let cname_record: DomainVerificationRecord = self.db.create("domain_verification_record", cname_record).await?;

        Ok(vec![txt_record, cname_record])
    }

    /// Get verification records for a domain
    async fn get_verification_records(
        &self,
        domain_id: &str,
    ) -> Result<Vec<DomainVerificationRecord>> {
        let query = format!(
            "SELECT * FROM domain_verification_record WHERE domain_id = '{}' ORDER BY created_at",
            domain_id
        );

        let mut response = self.db.query(&query).await?;
        let records: Vec<DomainVerificationRecord> = response.take(0)?;

        Ok(records)
    }

    /// Verify DNS record
    async fn verify_dns_record(
        &self,
        domain: &PublicationDomain,
        record: &DomainVerificationRecord,
    ) -> Result<bool> {
        match record.record_type.as_str() {
            "TXT" => self.verify_txt_record(&record.record_name, &record.record_value).await,
            "CNAME" => self.verify_cname_record(&record.record_name, &record.record_value).await,
            _ => Err(AppError::Internal(format!("Unsupported record type: {}", record.record_type))),
        }
    }

    /// Verify TXT record
    async fn verify_txt_record(&self, name: &str, expected_value: &str) -> Result<bool> {
        debug!("Verifying TXT record for {}", name);

        let lookup = self.dns_resolver.txt_lookup(name).await
            .map_err(|e| AppError::ExternalService(format!("DNS lookup failed: {}", e)))?;

        for record in lookup.iter() {
            for txt_data in record.iter() {
                let txt_string = std::str::from_utf8(txt_data)
                    .map_err(|e| AppError::Internal(format!("Invalid TXT record data: {}", e)))?;
                
                if txt_string == expected_value {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Verify CNAME record
    async fn verify_cname_record(&self, name: &str, expected_value: &str) -> Result<bool> {
        debug!("Verifying CNAME record for {}", name);

        let lookup = self.dns_resolver.lookup(name, trust_dns_resolver::proto::rr::RecordType::CNAME).await
            .map_err(|e| AppError::ExternalService(format!("DNS lookup failed: {}", e)))?;

        for record in lookup.iter() {
            if record.to_string().trim_end_matches('.') == expected_value.trim_end_matches('.') {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Update primary domain for a publication
    async fn update_primary_domain(
        &self,
        publication_id: &str,
        new_primary_id: &Uuid,
    ) -> Result<()> {
        // Remove primary flag from all other domains
        let query = format!(
            "UPDATE publication_domain SET is_primary = false WHERE publication_id = '{}' AND id != '{}'",
            publication_id, new_primary_id
        );

        self.db.query(&query).await?;
        Ok(())
    }

    /// Provision SSL certificate for a domain
    async fn provision_ssl_certificate(&self, domain_id: &str) -> Result<()> {
        debug!("Provisioning SSL certificate for domain {}", domain_id);

        let domain: PublicationDomain = self.db
            .get_by_id("publication_domain", domain_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Domain not found".to_string()))?;

        // Only provision for verified domains
        if !domain.is_verified() {
            return Err(AppError::BadRequest("Domain must be verified before SSL provisioning".to_string()));
        }

        let domain_name = match domain.domain_type {
            DomainType::Subdomain => domain.subdomain.as_ref(),
            DomainType::Custom => domain.custom_domain.as_ref(),
        }.ok_or_else(|| AppError::Internal("Domain name not found".to_string()))?;

        // Call SSL provider API if configured
        if let (Some(endpoint), Some(api_key)) = (&self.config.ssl_provider_endpoint, &self.config.ssl_provider_api_key) {
            let request_body = json!({
                "domain": domain_name,
                "type": "full",
                "webhook_url": self.config.ssl_webhook_url,
            });

            let response = self.http_client
                .post(endpoint)
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&request_body)
                .send()
                .await
                .map_err(|e| AppError::ExternalService(format!("SSL provider request failed: {}", e)))?;

            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                return Err(AppError::ExternalService(format!("SSL provisioning failed: {}", error_text)));
            }

            // Update SSL status
            let updates = json!({
                "ssl_status": SSLStatus::Pending,
                "updated_at": Utc::now(),
            });

            self.db.update_by_id_with_json::<PublicationDomain>(
                "publication_domain",
                domain_id,
                updates,
            ).await?;

            info!("SSL certificate provisioning initiated for domain {}", domain_name);
        } else {
            warn!("SSL provider not configured, skipping SSL provisioning");
        }

        Ok(())
    }

    /// Update SSL certificate status (called by webhook)
    pub async fn update_ssl_status(
        &self,
        domain_id: &str,
        status: SSLStatus,
        expires_at: Option<chrono::DateTime<Utc>>,
    ) -> Result<()> {
        debug!("Updating SSL status for domain {} to {:?}", domain_id, status);

        let updates = json!({
            "ssl_status": status,
            "ssl_expires_at": expires_at,
            "updated_at": Utc::now(),
        });

        self.db.update_by_id_with_json::<PublicationDomain>(
            "publication_domain",
            domain_id,
            updates,
        ).await?;

        Ok(())
    }

    /// Get domains needing SSL renewal
    pub async fn get_domains_needing_ssl_renewal(&self) -> Result<Vec<PublicationDomain>> {
        let query = format!(
            "SELECT * FROM publication_domain 
             WHERE ssl_status = 'active' 
             AND ssl_expires_at < '{}' 
             AND status = 'active'",
            Utc::now() + Duration::days(30)
        );

        let mut response = self.db.query(&query).await?;
        let domains: Vec<PublicationDomain> = response.take(0)?;

        Ok(domains)
    }

    /// Renew SSL certificates
    pub async fn renew_ssl_certificates(&self) -> Result<()> {
        let domains = self.get_domains_needing_ssl_renewal().await?;

        for domain in domains {
            match self.provision_ssl_certificate(&domain.id.to_string()).await {
                Ok(_) => info!("SSL renewal initiated for domain {}", domain.id),
                Err(e) => error!("Failed to renew SSL for domain {}: {}", domain.id, e),
            }
        }

        Ok(())
    }

    /// Get domain statistics
    pub async fn get_domain_stats(&self) -> Result<DomainStats> {
        let query = "
            SELECT 
                count() as total_domains,
                count(IF status = 'active' THEN 1 ELSE NULL END) as active_domains,
                count(IF status = 'pending' THEN 1 ELSE NULL END) as pending_domains,
                count(IF status = 'failed' THEN 1 ELSE NULL END) as failed_domains,
                count(IF ssl_status = 'active' THEN 1 ELSE NULL END) as ssl_active,
                count(IF ssl_status = 'pending' THEN 1 ELSE NULL END) as ssl_pending
            FROM publication_domain
        ";

        let mut response = self.db.query(query).await?;
        let stats: Vec<DomainStats> = response.take(0)?;

        stats.into_iter().next()
            .ok_or_else(|| AppError::Internal("Failed to get domain statistics".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subdomain_creation() {
        // Test subdomain creation logic
    }

    #[tokio::test]
    async fn test_custom_domain_verification() {
        // Test custom domain verification
    }

    #[tokio::test]
    async fn test_ssl_provisioning() {
        // Test SSL certificate provisioning
    }
}
