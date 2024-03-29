use anyhow::Result;
use mlua::{
    prelude::{LuaError, LuaMultiValue},
    Lua,
};
use std::{
    fs::read_to_string,
    path::{Component, Path, PathBuf},
};

#[macro_use]
pub mod r#async;
pub mod bot;
pub mod image;
pub mod os;
pub mod tags;

fn remove_upwards_components(path: &Path) -> PathBuf {
    let mut p = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => p.push("."),
            Component::Normal(c) => p.push(c),
            _ => {}
        }
    }

    p
}

pub fn include_lua<'a>(
    state: &'a Lua,
    root_path: &Path,
    path: &str,
) -> Result<Option<LuaMultiValue<'a>>> {
    let path = remove_upwards_components(Path::new(path));

    // Check if the path should be globbed
    if path
        .components()
        .any(|c| c.as_os_str() == "**" || c.as_os_str().to_string_lossy().starts_with("*."))
    {
        let path = root_path.join(path);
        let pattern = path.as_os_str().to_string_lossy().to_string();
        let paths = glob::glob(pattern.as_ref())?;

        for path in paths {
            let path = path?;
            let name = Path::new(&path).strip_prefix(root_path).unwrap();
            let source = read_to_string(&path)?;
            state
                .load(&source)
                .set_name(name.as_os_str().to_string_lossy())?
                .eval()?;
        }
    } else {
        let lua_path = path.clone();
        let path = root_path.join(path);

        let source = read_to_string(path)?;
        let result = state
            .load(&source)
            .set_name(lua_path.as_os_str().to_string_lossy())?
            .eval()?;

        return Ok(Some(result));
    }

    Ok(None)
}

pub fn lib_include(root_path: PathBuf, state: &Lua) -> Result<()> {
    let include_fn = state.create_function(move |state, path: String| {
        include_lua(state, root_path.as_path(), &path)
            .map_err(|err| {
                println!("error including \"{}\": {}", path, err.to_string());

                LuaError::SyntaxError {
                    message: err.to_string(),
                    incomplete_input: false,
                }
            })
            .map(|val| val.unwrap_or_default())
    })?;

    state.globals().set("include", include_fn)?;

    Ok(())
}
