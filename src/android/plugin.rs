use crate::core::plugin_api::Plugin;
use crate::core::Hachimi;
use egui::ahash::HashSet;
use filetime::FileTime;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;
use std::{fs, io, thread};

fn files_are_equal(p1: &Path, p2: &Path) -> io::Result<bool> {
    let meta1 = fs::metadata(p1)?;
    let meta2 = fs::metadata(p2)?;

    Ok(meta1.len() == meta2.len() && meta1.modified()? == meta2.modified()?)
}

fn do_copy(from: &Path, to: &Path) -> io::Result<()> {
    fs::copy(from, to)?;
    let meta = fs::metadata(from);
    if let Ok(meta) = meta {
        filetime::set_file_mtime(to, FileTime::from_last_modification_time(&meta))?;
    }
    Ok(())
}

fn get_so_files(plugin_dir: &PathBuf,) -> io::Result<HashSet<String>> {
    Ok(fs::read_dir(&plugin_dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if name.ends_with(".so") {
                Some(name)
            } else {
                None
            }
        })
        .collect())
}

fn get_plugins(external_plugin_dir: &PathBuf, private_plugin_dir: &PathBuf) -> io::Result<()> {
    if !fs::exists(&private_plugin_dir)? {
        fs::create_dir_all(&private_plugin_dir)?;
    }

    let external_plugins: HashSet<String> = get_so_files(&external_plugin_dir)?;

    let private_plugins: HashSet<String> = get_so_files(&private_plugin_dir)?;

    for plugin_name in &external_plugins {
        let external_path = external_plugin_dir.join(plugin_name);
        let private_path = private_plugin_dir.join(plugin_name);

        if !private_plugins.contains(plugin_name) {
            do_copy(&external_path, &private_path)?;
            info!("Copied {} to private dir", plugin_name);
        } else {
            if !files_are_equal(&external_path, &private_path)? {
                do_copy(&external_path, &private_path)?;
                info!("Overwritten {} in private dir", plugin_name);
            }
        }
    }

    for plugin_name in &private_plugins {
        if !external_plugins.contains(plugin_name) {
            let path = private_plugin_dir.join(plugin_name);
            fs::remove_file(&path)?;
            info!("Deleted {} from private dir", plugin_name);
        }
    }

    Ok(())
}

pub fn init(){
    thread::spawn(||{
        for plugin in load_plugins().iter() {
            info!("Initializing plugin: {}", plugin.name);
            let res = plugin.init();
            if !res.is_ok() {
                info!("Plugin init failed");
            }
        }
    });
}

fn load_plugins() -> Vec<Plugin> {
    let mut plugins = Vec::new();
    let plugin_dir = Path::new("/data/data")
        .join(&Hachimi::instance().game.package_name)
        .join("hachimi")
        .join("plugins");
    if let Err(e) = get_plugins(
        &Hachimi::instance().game.data_dir.join("plugins"),
        &plugin_dir,
    ) {
        error!("Failed to copy plugins: {e}");
    }
    for entry in fs::read_dir(&plugin_dir)
        .unwrap()
        .filter_map(|e| e.ok()) // 去掉错误的条目
        .filter(|e| e.path().extension().map(|ext| ext == "so").unwrap_or(false))
    {
        let path = entry.path();
        if let Some(path_str) = path.to_str() {
            unsafe {
                let handle = libc::dlopen(path_str.as_ptr(), libc::RTLD_LAZY);
                let hachimi_init_addr: usize =
                    std::mem::transmute(libc::dlsym(handle, c"hachimi_init".as_ptr()));
                info! {"Loaded plugin: {}", path_str}
                if hachimi_init_addr != 0 {
                    if let Some(name) = path.file_name() {
                        plugins.push(Plugin {
                            name: name.to_str().unwrap().to_string(),
                            init_fn: std::mem::transmute(hachimi_init_addr),
                        });
                    }
                }
            }
        }
    }
    plugins
}
