//! Whether a command goes through a proxy, which one, and whether it may go directly after all (Agent/Client.hs `shouldUseProxy`, `directAllowed`, `getNextServer`).
use crate::agent::client::{AgentClient, AgentConfig};
use crate::crypto::random_array;
use crate::smp::SmpServer;
use crate::types::{ProxyFallback, ProxyMode};

/// `ipAddressProtected`: only a SOCKS proxy or onion hosts hide the address, and this client
/// has neither.
const fn ip_address_protected(_: &SmpServer) -> bool {
    false
}

/// `chooseTransportHost` under the default host mode without SOCKS: the first public host, or
/// the first host when all are onion.
fn transport_host(s: &SmpServer) -> Option<&str> {
    let public = s.hosts.iter().find(|h| !h.ends_with(".onion"));
    public.or_else(|| s.hosts.first()).map(String::as_str)
}

impl AgentClient {
    pub(crate) fn should_proxy(&self, dest: &SmpServer) -> bool {
        match self.cfg.proxy_mode {
            ProxyMode::Always => true,
            ProxyMode::Unknown => !self.known(dest),
            ProxyMode::Unprotected => !ip_address_protected(dest) && !self.known(dest),
            ProxyMode::Never => false,
        }
    }

    /// May a command the proxy could not forward go to `dest` directly?
    pub(crate) fn direct_allowed(&self, dest: &SmpServer) -> bool {
        match self.cfg.proxy_fallback {
            ProxyFallback::Allow => true,
            ProxyFallback::AllowProtected => ip_address_protected(dest),
            ProxyFallback::Prohibit => false,
        }
    }

    /// `knownHosts`: a relay of ours already sees our address, so it learns nothing new.
    fn known(&self, dest: &SmpServer) -> bool {
        let Some(host) = transport_host(dest) else {
            return false;
        };
        self.cfg
            .servers
            .iter()
            .any(|s| s.hosts.iter().any(|h| h == host))
    }

    /// The proxy `dest` goes through: the one chosen before, or a new pick.
    pub(crate) fn proxy_for(&mut self, dest: &SmpServer) -> Option<SmpServer> {
        if let Some(p) = self.proxies.chosen.get(dest) {
            return Some(p.clone());
        }
        let p = pick(&self.cfg, dest)?;
        self.proxies.chosen.insert(dest.clone(), p.clone());
        Some(p)
    }
}

/// `getNextServer` over the enabled servers (all of them when none is): at random among those
/// that share no host with `dest`, or among all when none is left, as the reference does. The
/// reference first prefers another operator; servers here carry none.
fn pick(cfg: &AgentConfig, dest: &SmpServer) -> Option<SmpServer> {
    let enabled: Vec<&SmpServer> = cfg
        .servers
        .iter()
        .filter(|s| !cfg.disabled.contains(s))
        .collect();
    let pool = if enabled.is_empty() {
        cfg.servers.iter().collect()
    } else {
        enabled
    };
    let apart: Vec<&SmpServer> = pool
        .iter()
        .copied()
        .filter(|s| s.hosts.iter().all(|h| !dest.hosts.contains(h)))
        .collect();
    let from = if apart.is_empty() { pool } else { apart };
    let r = u64::from_le_bytes(random_array().ok()?);
    let i = usize::try_from(r % u64::try_from(from.len()).ok()?.max(1)).ok()?;
    from.get(i).map(|s| (*s).clone())
}
