use serde::{Deserialize, Serialize};

crate::define_music_item_struct_with_common_fields!(
    Album, crate::models::ItemType::Album, "album",
    {
        #[serde(default)]
        artist_ids: Vec<String>,
    }
);
