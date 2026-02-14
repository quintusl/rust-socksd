use super::Authenticator;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use ldap3::{LdapConnAsync, Scope, SearchEntry};
use tracing::{debug, error};

#[derive(Clone)]
pub struct LdapAuthenticator {
    url: String,
    base_dn: String,
    bind_dn: Option<String>,
    bind_password: Option<String>,
    user_filter: String,
}

impl LdapAuthenticator {
    pub fn new(
        url: &str, 
        base_dn: &str, 
        bind_dn: Option<String>, 
        bind_password: Option<String>,
        user_filter: &str
    ) -> Self {
        Self {
            url: url.to_string(),
            base_dn: base_dn.to_string(),
            bind_dn,
            bind_password,
            user_filter: user_filter.to_string(),
        }
    }
    
    fn escape_filter_value(value: &str) -> String {
        value.replace('\\', "\\5c")
             .replace('*', "\\2a")
             .replace('(', "\\28")
             .replace(')', "\\29")
             .replace('\0', "\\00")
    }
}

#[async_trait]
impl Authenticator for LdapAuthenticator {
    async fn authenticate(&self, username: &str, password: &str) -> Result<bool> {
        let (conn, mut ldap) = LdapConnAsync::new(&self.url).await
            .map_err(|e| anyhow!("Failed to connect to LDAP server: {}", e))?;
            
        ldap3::drive!(conn);

        // 1. Bind to search for the user
        let bind_result = if let Some(bind_dn) = &self.bind_dn {
            let bind_pw = self.bind_password.as_deref().unwrap_or("");
            ldap.simple_bind(bind_dn, bind_pw).await
        } else {
            ldap.simple_bind("", "").await
        };
        
        if let Err(e) = bind_result {
             error!("LDAP initial bind failed: {}", e);
             return Err(anyhow!("LDAP bind failed: {}", e));
        }
        
        if let Ok(res) = bind_result {
            if let Err(e) = res.success() {
                 error!("LDAP initial bind error result: {}", e);
                 return Err(anyhow!("LDAP bind error: {}", e));
            }
        }

        // 2. Search for the user DN
        let safe_username = Self::escape_filter_value(username);
        let filter = self.user_filter.replace("{}", &safe_username);
        
        let search_result = ldap.search(
            &self.base_dn,
            Scope::Subtree,
            &filter,
            vec!["dn"]
        ).await;
        
        let (rs, _res) = match search_result {
            Ok(res) => res.success().map_err(|e| anyhow!("LDAP search error: {}", e))?,
            Err(e) => return Err(anyhow!("LDAP search failed: {}", e)),
        };

        if rs.is_empty() {
            debug!("LDAP user not found: {}", username);
            return Ok(false);
        }
        
        if rs.len() > 1 {
            debug!("LDAP user ambiguous (multiple matches): {}", username);
            return Ok(false);
        }

        let user_dn = SearchEntry::construct(rs[0].clone()).dn;
        debug!("Found LDAP user DN: {}", user_dn);

        // 3. Verify password by binding as the user
        // We can rebind the existing connection
        let verify_result = ldap.simple_bind(&user_dn, password).await;

        match verify_result {
            Ok(res) => {
                let success = res.success().is_ok();
                if !success {
                    debug!("LDAP password verification failed for {}", username);
                }
                Ok(success)
            },
            Err(e) => {
                debug!("LDAP bind error for user {}: {}", username, e);
                Ok(false)
            }
        }
    }
}
