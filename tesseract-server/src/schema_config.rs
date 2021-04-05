use anyhow::{Error, format_err};

use tesseract_core::Schema;


/// Reads a schema from an XML or JSON file and converts it into a `tesseract_core::Schema` object.
pub fn read_schema(schema_path: &str) -> Result<Schema, Error> {
    let schema_str = std::fs::read_to_string(&schema_path)
        .map_err(|_| format_err!("Schema file not found at {}", schema_path))?;

    let schema = if schema_path.ends_with("xml") {
        Schema::from_xml(&schema_str)?
    } else if schema_path.ends_with("json") {
        Schema::from_json(&schema_str)?
    } else {
        return Err(format_err!("Schema format not supported"))
    };

    // TODO Should this check be done in core?
    for cube in &schema.cubes {
        for dimension in &cube.dimensions {
            for hierarchy in &dimension.hierarchies {
                let has_table = hierarchy.table.is_some();
                let has_inline_table = hierarchy.inline_table.is_some();

                if has_table && has_inline_table {
                    return Err(format_err!("Can't have table and inline table definitions in the same hierarchy"))
                }
            }
        }
    }

    Ok(schema)
}
