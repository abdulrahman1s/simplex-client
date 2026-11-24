//! Private message routing settings (simplexmq Client.hs `SMPProxyMode`, `SMPProxyFallback`; simplex-chat 7.1.4 `Terminal.hs` defaults).

use std::fmt;
use std::str::FromStr;

/// When messages go through a relay of our own instead of straight to the recipient's relay,
/// which would see our IP address.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProxyMode {
    /// Always through a proxy.
    Always,
    /// Through a proxy when the recipient's relay is not one of ours. The chat's default.
    #[default]
    Unknown,
    /// As `Unknown`, unless the IP address is hidden by SOCKS or onion hosts, which this client
    /// does not use, so here it is the same as `Unknown`.
    Unprotected,
    /// Never.
    Never,
}

/// Whether a message may go straight to the recipient's relay when the proxy or that relay
/// does not support forwarding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProxyFallback {
    Allow,
    /// Only when the IP address is hidden, which this client never does, so never. The chat's default.
    #[default]
    AllowProtected,
    Prohibit,
}

impl ProxyMode {
    /// The setting's name in the chat's options (`strEncode`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Unknown => "unknown",
            Self::Unprotected => "unprotected",
            Self::Never => "never",
        }
    }
}

impl ProxyFallback {
    /// The setting's name in the chat's options (`strEncode`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "yes",
            Self::AllowProtected => "protected",
            Self::Prohibit => "no",
        }
    }
}

/// A name `as_str` does not give.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadSetting(pub String);

impl fmt::Display for BadSetting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown routing setting: {}", self.0)
    }
}

impl std::error::Error for BadSetting {}

impl FromStr for ProxyMode {
    type Err = BadSetting;

    fn from_str(s: &str) -> Result<Self, BadSetting> {
        [Self::Always, Self::Unknown, Self::Unprotected, Self::Never]
            .into_iter()
            .find(|m| m.as_str() == s)
            .ok_or_else(|| BadSetting(s.to_owned()))
    }
}

impl FromStr for ProxyFallback {
    type Err = BadSetting;

    fn from_str(s: &str) -> Result<Self, BadSetting> {
        [Self::Allow, Self::AllowProtected, Self::Prohibit]
            .into_iter()
            .find(|m| m.as_str() == s)
            .ok_or_else(|| BadSetting(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_defaults_are_the_chat_terminal_defaults() {
        assert_eq!(ProxyMode::default(), ProxyMode::Unknown);
        assert_eq!(ProxyFallback::default(), ProxyFallback::AllowProtected);
    }

    #[test]
    fn routing_settings_read_back_their_option_names() {
        for m in ["always", "unknown", "unprotected", "never"] {
            assert_eq!(m.parse::<ProxyMode>().unwrap().as_str(), m);
        }
        for f in ["yes", "protected", "no"] {
            assert_eq!(f.parse::<ProxyFallback>().unwrap().as_str(), f);
        }
        assert!("sometimes".parse::<ProxyMode>().is_err());
    }
}
