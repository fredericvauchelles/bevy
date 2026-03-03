use alloc::borrow::Cow;
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{PoisonError, RwLock};
use std::time::SystemTime;
use toml_edit::Document;

#[derive(Clone, Debug)]
pub struct BevyManifestConfig {
    pub reexports: Cow<'static, [Cow<'static, str>]>,
    pub prefixes: Cow<'static, [Cow<'static, str>]>,
    source: Option<(PathBuf, SystemTime)>,
}

impl BevyManifestConfig {
    pub fn get() -> BevyManifestConfig {
        static CONFIGS: RwLock<BTreeMap<PathBuf, BevyManifestConfig>> =
            RwLock::new(BTreeMap::new());

        let manifest_dir =
            env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be defined");
        let manifest_dir = Path::new(&manifest_dir);

        {
            let configs = CONFIGS.read().unwrap_or_else(PoisonError::into_inner);
            if let Some(found_config) = configs.get(manifest_dir).or_else(|| {
                configs.iter().find_map(|(config_path_root, config)| {
                    if manifest_dir.starts_with(config_path_root) {
                        Some(config)
                    } else {
                        None
                    }
                })
            }) && let Some((config_path, modified_time)) = found_config.source.as_ref()
            {
                let new_modified_time = std::fs::metadata(&config_path)
                    .and_then(|metadata| metadata.modified())
                    .expect("Modified time must be available");
                if *modified_time == new_modified_time {
                    return found_config.clone();
                }
            }
        }

        let mut pivot_path = Some(manifest_dir);
        while let Some(path) = pivot_path {
            let config_path = Self::path_to_config(&path);
            if config_path.exists() {
                let modified_time = std::fs::metadata(&config_path)
                    .and_then(|metadata| metadata.modified())
                    .expect("Modified time must be available");
                let config_str = std::fs::read_to_string(&config_path)
                    .unwrap_or_else(|_| panic!("Failed to read {config_path:?}"));
                let config_doc = Document::parse(&*config_str)
                    .unwrap_or_else(|err| panic!("Failed to parse {config_path:?}: {err}"));

                let new_config = BevyManifestConfig {
                    prefixes: config_member_to_string_array("prefixes", &config_path, &config_doc),
                    reexports: config_member_to_string_array(
                        "reexports",
                        &config_path,
                        &config_doc,
                    ),
                    source: Some((config_path, modified_time)),
                };

                let mut write = CONFIGS.write().unwrap_or_else(PoisonError::into_inner);
                write.insert(path.to_path_buf(), new_config.clone());
                write.insert(manifest_dir.to_path_buf(), new_config.clone());

                return new_config;
            } else {
                pivot_path = path.parent();
            }
        }

        // no config file found, use a default config
        BevyManifestConfig {
            reexports: Cow::Borrowed(&[Cow::Borrowed("bevy")]),
            prefixes: Cow::Borrowed(&[Cow::Borrowed("bevy_")]),
            source: None,
        }
    }

    fn path_to_config(root: &Path) -> PathBuf {
        root.join(".config").join("bevy_manifest.toml")
    }
}

fn config_member_to_string_array(
    member: &str,
    config_path: &Path,
    config_doc: &Document<&str>,
) -> Cow<'static, [Cow<'static, str>]> {
    config_doc
        .get("macros")
        .and_then(|w| w.get(member))
        .map(|a| {
            a.as_array()
                .expect(&format!(
                    "'macros.{member}' must be a [str] in {config_path:?}"
                ))
                .iter()
                .map(|t| {
                    Cow::Owned(
                        t.as_str()
                            .expect(&format!(
                                "'macros.{member}' must be a [str] in {config_path:?}"
                            ))
                            .to_string(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![Cow::Borrowed("bevy_")])
        .into()
}
