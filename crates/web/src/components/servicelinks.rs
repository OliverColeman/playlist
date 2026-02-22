use dioxus::prelude::*;
use playlist_core::models::{ExternalServiceAssociation, ItemType, MusicItem};

const SPOTIFY_ICON: Asset = asset!("/assets/external_service_icons/spotify.png");
const MUSICBRAINZ_ICON: Asset = asset!("/assets/external_service_icons/musicbrainz.png");
const YOUTUBE_ICON: Asset = asset!("/assets/external_service_icons/youtube.png");

#[component]
pub fn ServiceLinks<T: MusicItem + PartialEq + 'static>(music_item: T) -> Element {
    rsx! {
        div { class: "flex gap-1",
            YouTubeLink { music_item: music_item.clone() }
            match music_item.external_service_associations() {
                Some(associations) => rsx! {
                    for service_association in associations.iter() {
                        match service_association {
                            ExternalServiceAssociation::Spotify { id, image_urls: _ } => rsx! {
                                SpotifyLink { id: id.clone(), item_type: T::item_type() }
                            },
                            ExternalServiceAssociation::MusicBrainz { id } => rsx! {
                                MusicBrainzLink { id: id.clone(), item_type: T::item_type() }
                            },
                        }
                    }
                },
                None => rsx! {},
            }
        }
    }
}

#[component]
pub fn SpotifyLink(id: String, item_type: ItemType) -> Element {
    let url = match item_type {
        ItemType::Track => format!("https://open.spotify.com/track/{}", id),
        ItemType::Artist => format!("https://open.spotify.com/artist/{}", id),
        ItemType::Album => format!("https://open.spotify.com/album/{}", id),
        ItemType::Playlist => format!("https://open.spotify.com/playlist/{}", id),
        ItemType::Compiler => format!("https://open.spotify.com/user/{}", id),
    };
    rsx! {
        a {
            href: "{url}",
            target: "_blank",
            rel: "noopener noreferrer",
            class: "service-icon",
            style: "background-image: url({SPOTIFY_ICON}); background-size: contain; background-repeat: no-repeat; background-position: center;",
        }
    }
}

#[component]
pub fn MusicBrainzLink(id: String, item_type: ItemType) -> Element {
    let url = match item_type {
        ItemType::Track => format!("https://musicbrainz.org/recording/{}", id),
        ItemType::Artist => format!("https://musicbrainz.org/artist/{}", id),
        ItemType::Album => format!("https://musicbrainz.org/release-group/{}", id),
        _ => "".to_string(),
    };
    rsx! {
        a {
            href: "{url}",
            target: "_blank",
            rel: "noopener noreferrer",
            class: "service-icon",
            style: "background-image: url({MUSICBRAINZ_ICON}); background-size: contain; background-repeat: no-repeat; background-position: center;",
        }
    }
}

#[component]
pub fn YouTubeLink<T: MusicItem + PartialEq + 'static>(music_item: T) -> Element {
    // Only for track, artist, album types
    if !matches!(
        T::item_type(),
        ItemType::Track | ItemType::Artist | ItemType::Album
    ) {
        return rsx! {};
    }

    let url = format!(
        "https://www.youtube.com/results?search_query={}",
        music_item.name()
    );
    rsx! {
        a {
            href: "{url}",
            target: "_blank",
            rel: "noopener noreferrer",
            class: "service-icon",
            style: "background-image: url({YOUTUBE_ICON}); background-size: contain; background-repeat: no-repeat; background-position: center;",
        }
    }
}
