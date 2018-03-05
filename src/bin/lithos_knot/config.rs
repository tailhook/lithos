use std::path::Path;

use lithos::container_config::ContainerConfig;
use lithos::child_config::ChildInstance;
use lithos::utils::temporary_change_root;

use quire::{parse_config, Options};


pub fn container_config(root: &Path, child_cfg: &ChildInstance)
    -> Result<ContainerConfig, String>
{
    return temporary_change_root(root, || {
        parse_config(&child_cfg.config,
            &ContainerConfig::validator(), &Options::default())
        .map_err(|e| e.to_string())
    });
}
