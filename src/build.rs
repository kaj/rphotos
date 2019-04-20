use ructe::{Ructe, RucteError};

fn main() -> Result<(), RucteError> {
    let mut ructe = Ructe::from_env()?;
    let mut statics = ructe.statics()?;
    statics.add_sass_file("res/photos.scss")?;
    statics.add_file("res/admin.js")?;
    statics.add_file("res/ux.js")?;
    statics.add_files_as("res/leaflet-1.3.1", "l131")?;
    statics.add_files_as("res/leaflet-cluster-1.3.0", "lm130")?;
    ructe.compile_templates("templates")?;
    Ok(())
}
