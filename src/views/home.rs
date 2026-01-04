use dioxus::prelude::*;

use crate::components;
use crate::music_data;
use crate::views::playlist::PlaylistListComp;

/// The Home page component that will be rendered when the current route is `[Route::Home]`
#[component]
pub fn Home() -> Element {
    let playlists_by_id =
        use_context::<Resource<music_data::MusicItemsById<music_data::playlist::PlayList>>>();

    rsx! {
        div { class: "text-center mb-[2em]",
            h1 { "The Just Dance Playlist Archives" }
            p {
                "Just Dance is a free-movement, loosely facilitated dance session in Newcastle, Australia."
            }
            p {
                " Loosen up, let go, and just dance! More info on the "
                a { href: "https://www.facebook.com/groups/166298646868224/about",
                    "Facebook group"
                }
            }
        }
        div { class: "lg:w-4xl mx-auto",
            div {
                match &*playlists_by_id.read_unchecked() {
                    Some(playlists) => rsx! {
                        PlaylistListComp { playlists: playlists.clone(), hide_compiler: false }
                    },
                    None => rsx! {
                        p { components::Loading {} }
                    },
                }
            }
        }
    }
}
