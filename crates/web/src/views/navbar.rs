use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn Navbar() -> Element {
    rsx! {
        div {
            class: "flex flex-row justify-center gap-2 font-bold",
            id: "navbar",
            Link { to: Route::Home {}, "Playlists" }
            Link { to: Route::CompilerListComp {}, "Compilers" }
            Link { to: Route::PopularTracksComp {}, "Top 100" }
            Link { to: Route::SearchComp {}, "Search" }
        }

        Outlet::<Route> {}
    }
}
