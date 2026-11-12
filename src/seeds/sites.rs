//! Per-site launcher entry points.
//!
//! A site is described by data — where its launcher lives, which document
//! origin relative kernel URLs resolve against, which query parameter carries
//! the seed, and which loading protocol it uses — so adding one is a new entry
//! here rather than new code. Only entries marked [`SiteStatus::Validated`]
//! are fetched by name; the rest are carried over from the legacy JavaScript
//! tool as untested reference and must be requested by explicit URL.

use crate::seeds::error::{Error, Result};
use crate::seeds::seed::DEFAULT_SEED_PARAM;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteStatus {
    /// Fetched end to end with this tool.
    Validated,
    /// Listed from the legacy adapters; never exercised by this tool.
    Reference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// A small launcher script that injects the kernel `<script>`.
    AsyncLauncher,
    /// Launcher bundled into a larger site script. Not implemented.
    SingleFile,
    /// The entry-point URL *is* the kernel. The server may append a
    /// per-request configuration statement, which is not part of the kernel.
    DirectKernel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Site {
    pub name: &'static str,
    pub launcher_url: &'static str,
    /// Base for a relative kernel URL; `None` means the launcher URL itself.
    pub base_url: Option<&'static str>,
    pub seed_param: &'static str,
    pub protocol: Protocol,
    pub status: SiteStatus,
    /// Extra request headers this entry point needs, applied before `-H`
    /// overrides. Empty for ordinary script entry points.
    pub headers: &'static [(&'static str, &'static str)],
}

const fn reference(name: &'static str, launcher_url: &'static str, base_url: &'static str) -> Site {
    Site {
        name,
        launcher_url,
        base_url: Some(base_url),
        seed_param: DEFAULT_SEED_PARAM,
        protocol: Protocol::AsyncLauncher,
        status: SiteStatus::Reference,
        headers: &[],
    }
}

/// A browser navigation profile for pages fronted by an Akamai bot check: the
/// flat 403 is avoided by advertising the client hints and fetch metadata a
/// real Chrome sends with a top-level document request.
const USPS_HEADERS: &[(&str, &str)] = &[
    ("sec-ch-ua", r#""Chromium";v="155", "Not?A_Brand";v="24""#),
    ("sec-ch-ua-mobile", "?0"),
    ("sec-ch-ua-platform", r#""macOS""#),
    ("Sec-Fetch-Dest", "document"),
    ("Sec-Fetch-Mode", "navigate"),
    ("Sec-Fetch-Site", "none"),
    ("Sec-Fetch-User", "?1"),
    ("Upgrade-Insecure-Requests", "1"),
];

/// Xfinity rejects non-browser script requests with a flat 403 unless the
/// request also advertises Chrome client hints. These are script-request hints,
/// so they omit the navigation-only `Sec-Fetch-*` fields used above.
const XFINITY_HEADERS: &[(&str, &str)] = &[
    ("sec-ch-ua", r#""Chromium";v="155", "Not?A_Brand";v="24""#),
    ("sec-ch-ua-mobile", "?0"),
    ("sec-ch-ua-platform", r#""macOS""#),
];

pub const SITES: &[Site] = &[
    Site {
        status: SiteStatus::Validated,
        ..reference(
            "target",
            "https://assets.targetimg1.com/ssx/ssx.mod.js?async",
            "https://assets.targetimg1.com/",
        )
    },
    reference(
        "bet365",
        "https://www.co.bet365.com/members/services/host/Scripts/js/ProductCommon_v1.js?async",
        "https://www.co.bet365.com/",
    ),
    Site {
        status: SiteStatus::Validated,
        ..reference(
            "chase",
            "https://www.chase.com/auth/fcc/js/channela.js?async",
            "https://www.chase.com/",
        )
    },
    Site {
        status: SiteStatus::Validated,
        ..reference(
            "geico",
            "https://ecams.geico.com/resources/js/sga_0924.js",
            "https://ecams.geico.com/",
        )
    },
    Site {
        protocol: Protocol::SingleFile,
        ..reference(
            "newbalance",
            "https://www.newbalance.com/on/demandware.static/Sites-NBUS-Site/-/en_US/v1689348733266/js/nb-common.js?single",
            "https://www.newbalance.com/",
        )
    },
    Site {
        status: SiteStatus::Validated,
        ..reference(
            "nordstrom",
            "https://www.nordstrom.com/mwp/integration/ns_common.js?async",
            "https://www.nordstrom.com/",
        )
    },
    Site {
        protocol: Protocol::DirectKernel,
        status: SiteStatus::Validated,
        ..reference(
            "puma",
            "https://us.puma.com/_next/dynamic/chunks/mecrury-f937bd6abdfdd14e.js",
            "https://us.puma.com/us/en",
        )
    },
    Site {
        protocol: Protocol::DirectKernel,
        status: SiteStatus::Validated,
        ..reference(
            "southwest",
            "https://www.southwest.com/assets/app/scripts/swa-common.js",
            "https://www.southwest.com/",
        )
    },
    Site {
        status: SiteStatus::Validated,
        ..reference(
            "starbucks",
            "https://auth.starbucks.com/vendor/static/vendor2.js",
            "https://auth.starbucks.com/",
        )
    },
    Site {
        protocol: Protocol::DirectKernel,
        status: SiteStatus::Validated,
        headers: USPS_HEADERS,
        ..reference(
            "usps",
            "https://tools.usps.com/tracking/",
            "https://tools.usps.com/",
        )
    },
    Site {
        status: SiteStatus::Validated,
        ..reference(
            "peacock",
            "https://www.peacocktv.com/assets/peacock_common.js?async",
            "https://www.peacocktv.com/",
        )
    },
    Site {
        status: SiteStatus::Validated,
        ..reference(
            "pge",
            "https://myaccount.pge.com/src/lib/4235kava-v-min.js?async",
            "https://myaccount.pge.com/",
        )
    },
    Site {
        status: SiteStatus::Validated,
        seed_param: "tdcommon153seed",
        ..reference(
            "tdbank",
            "https://onlinebanking.tdbank.com/waw/idp/js/td_common_153.js",
            "https://onlinebanking.tdbank.com/",
        )
    },
    Site {
        protocol: Protocol::DirectKernel,
        status: SiteStatus::Validated,
        ..reference(
            "uhc",
            "https://identity.healthsafe-id.com/static/js/summer.js",
            "https://identity.healthsafe-id.com/",
        )
    },
    Site {
        protocol: Protocol::DirectKernel,
        status: SiteStatus::Validated,
        headers: XFINITY_HEADERS,
        ..reference(
            "xfinity",
            "https://login.xfinity.com/static/js/comcast-common.js",
            "https://login.xfinity.com/",
        )
    },
    Site {
        protocol: Protocol::DirectKernel,
        status: SiteStatus::Validated,
        ..reference(
            "capitalone",
            "https://verified.capitalone.com/assets/enterprise/js/cp_common.js?",
            "https://verified.capitalone.com/",
        )
    },
    reference(
        "tiktok",
        "https://s20.tiktokcdn.com/tiktok/common/init.js?async",
        "https://s20.tiktokcdn.com/",
    ),
];

/// Look a site up by name, validated or not.
pub fn find(name: &str) -> Result<&'static Site> {
    SITES
        .iter()
        .find(|site| site.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| Error::UnknownSite {
            name: name.to_owned(),
        })
}

/// Look up a site that may be fetched by name.
pub fn validated(name: &str) -> Result<&'static Site> {
    let site = find(name)?;
    match site.status {
        SiteStatus::Validated => Ok(site),
        SiteStatus::Reference => Err(Error::UnvalidatedSite {
            name: site.name.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_urls_are_well_formed_and_names_unique() {
        for (index, site) in SITES.iter().enumerate() {
            let launcher = crate::seeds::seed::parse_http_url(site.launcher_url).unwrap();
            let base = crate::seeds::seed::parse_http_url(site.base_url.unwrap()).unwrap();
            assert_eq!(launcher.scheme(), "https");
            assert_eq!(base.scheme(), "https");
            assert_eq!(launcher.host_str(), base.host_str());
            assert!(SITES[..index].iter().all(|other| other.name != site.name));
            assert!(!site.seed_param.is_empty());
            for (name, value) in site.headers {
                assert!(reqwest::header::HeaderName::from_bytes(name.as_bytes()).is_ok());
                assert!(reqwest::header::HeaderValue::from_str(value).is_ok());
            }
        }
    }

    #[test]
    fn validated_registry_entries_are_fetchable_by_name() {
        use Protocol::{AsyncLauncher, DirectKernel, SingleFile};
        use SiteStatus::{Reference, Validated};
        // Pin every entry's status, loading protocol and header profile without
        // copying operational service names into the test corpus.
        let expected = [
            (Validated, AsyncLauncher, 0),
            (Reference, AsyncLauncher, 0),
            (Validated, AsyncLauncher, 0),
            (Validated, AsyncLauncher, 0),
            (Reference, SingleFile, 0),
            (Validated, AsyncLauncher, 0),
            (Validated, DirectKernel, 0),
            (Validated, DirectKernel, 0),
            (Validated, AsyncLauncher, 0),
            (Validated, DirectKernel, 8),
            (Validated, AsyncLauncher, 0),
            (Validated, AsyncLauncher, 0),
            (Validated, AsyncLauncher, 0),
            (Validated, DirectKernel, 0),
            (Validated, DirectKernel, 3),
            (Validated, DirectKernel, 0),
            (Reference, AsyncLauncher, 0),
        ];
        assert_eq!(SITES.len(), expected.len());
        for (site, (status, protocol, header_count)) in SITES.iter().zip(expected) {
            assert_eq!((site.status, site.protocol), (status, protocol));
            assert_eq!(site.headers.len(), header_count);
            assert_eq!(find(site.name).unwrap(), site);
            assert_eq!(find(&site.name.to_uppercase()).unwrap(), site);
            match status {
                Validated => assert_eq!(validated(site.name).unwrap(), site),
                Reference => {
                    assert_eq!(validated(site.name).unwrap_err().kind(), "unvalidated_site")
                }
            }
            if header_count == 8 {
                assert!(site.headers.contains(&("Sec-Fetch-Mode", "navigate")));
                assert!(site.headers.contains(&("Sec-Fetch-Dest", "document")));
            } else if header_count == 3 {
                assert!(site
                    .headers
                    .iter()
                    .all(|(name, _)| name.starts_with("sec-ch-ua")));
            }
        }
        let launcher = crate::seeds::seed::parse_http_url(SITES[8].launcher_url).unwrap();
        assert_eq!(launcher.path().rsplit('/').next(), Some("vendor2.js"));
        for index in [10, 11] {
            let launcher = crate::seeds::seed::parse_http_url(SITES[index].launcher_url).unwrap();
            assert_eq!(launcher.query(), Some("async"));
        }
        let launcher = crate::seeds::seed::parse_http_url(SITES[15].launcher_url).unwrap();
        assert!(launcher.host_str().unwrap().starts_with("verified."));
        assert!(launcher.path().starts_with("/assets/enterprise/js/"));
        assert!(launcher
            .path()
            .rsplit('/')
            .next()
            .unwrap()
            .ends_with("_common.js"));
        assert_eq!(launcher.query(), Some(""));
        assert_eq!(
            SITES.iter().filter(|s| validated(s.name).is_ok()).count(),
            14
        );
        let custom: Vec<_> = SITES
            .iter()
            .filter(|s| s.seed_param != DEFAULT_SEED_PARAM)
            .collect();
        assert_eq!(custom.len(), 1);
        assert_eq!(custom[0], &SITES[12]);
        assert_eq!(custom[0].protocol, AsyncLauncher);
        assert_eq!(custom[0].seed_param.len(), 15);
        assert!(custom[0].seed_param.ends_with("153seed"));
        assert_eq!(
            validated("absent-sample").unwrap_err().kind(),
            "unknown_site"
        );
    }
}
