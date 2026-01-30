use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::models::album::Album;
use crate::models::artist::Artist;

crate::define_music_item_struct_with_common_fields!(
    Track, "track",
    {
        #[serde(default)]
        artist_ids: Vec<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        album_id: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        duration: Option<f64>,
    }
);

/// A linked track document is used to group different versions of the same track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedTrack {
    #[serde(rename = "_id")]
    pub id: String,
    pub track_name_normalised_strong: String,
    pub track_ids: Vec<String>,
    pub artist_ids: Vec<String>,
}

#[cfg(feature = "server")]
pub async fn load_linked_tracks(
    query: mongodb::bson::Document,
) -> Result<Vec<LinkedTrack>, crate::database::ServerError> {
    use futures::stream::TryStreamExt;

    let database = crate::get_database().await?;
    let collection: mongodb::Collection<LinkedTrack> = database.collection("linked_track");

    // Create index for track_ids queries (won't hurt if it already exists)
    let index_model = mongodb::IndexModel::builder()
        .keys(mongodb::bson::doc! {"track_ids": 1})
        .build();
    collection.create_index(index_model).await?;

    let cursor = collection.find(query).await?;
    let linked_tracks: Vec<LinkedTrack> = cursor.try_collect().await?;
    Ok(linked_tracks)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackWithAssociatedData {
    pub linked_tracks_by_id: HashMap<String, Track>,
    pub artists_by_id: HashMap<String, Artist>,
    pub albums_by_id: HashMap<String, Album>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackListWithAssociatedData {
    pub sorted_track_ids: Vec<String>,
    pub tracks_by_id: HashMap<String, Track>,
    pub linked_tracks: Vec<HashSet<String>>,
    pub artists_by_id: HashMap<String, Artist>,
    pub albums_by_id: HashMap<String, Album>,
}
