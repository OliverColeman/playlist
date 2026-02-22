use serde::{Deserialize, Serialize};

crate::define_music_item_struct_with_common_fields!(
    Compiler,
    crate::models::ItemType::Compiler,
    "compiler",
    {} // No additional fields
);
