use dioxus::prelude::*;

#[component]
pub fn Loading() -> Element {
    rsx! {
        // We can create elements inside the rsx macro with the element name followed by a block of attributes and children.
        span {
            class: "loading inline-flex align-middle leading-none font-bold p-[2px]",
            class: "before:content-['♫'] before:block before:animate-spin",
        }
    }
}

#[component]
pub fn IconsAndLinks() -> Element {
    rsx! { "TODO" }
}
