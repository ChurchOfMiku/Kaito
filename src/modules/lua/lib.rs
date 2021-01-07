use anyhow::Result;
use mlua::{
    prelude::{LuaError, LuaMultiValue},
    Lua,
};
use std::{
    fs::read_to_string,
    path::{Component, Path, PathBuf},
};

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
        let pattern = path.as_os_str().to_string_lossy();
        let paths = glob::glob(pattern.as_ref())?;

        for path in paths {
            let path = path?;
            let source = read_to_string(&path)?;
            state
                .load(&source)
                .set_name(path.as_os_str().to_string_lossy().as_bytes())?
                .eval()?;
        }
    } else {
        let lua_path = path.clone();
        let path = root_path.join(path);

        let source = read_to_string(path)?;
        let result = state
            .load(&source)
            .set_name(lua_path.as_os_str().to_string_lossy().as_bytes())?
            .eval()?;

        return Ok(Some(result));
    }

    Ok(None)
}

#[macro_use]
pub mod r#async;
pub mod os;

pub fn lib_include(root_path: PathBuf, state: &Lua) -> Result<()> {
    let include_fn = state.create_function(move |state, path: String| {
        include_lua(state, root_path.as_path(), &path)
            .map_err(|err| LuaError::RuntimeError(err.to_string()))
            .map(|val| val.unwrap_or_default())
    })?;

    state.globals().set("include", include_fn)?;

    Ok(())
}
