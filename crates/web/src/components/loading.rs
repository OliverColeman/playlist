use dioxus::prelude::*;

#[component]
pub fn Loading() -> Element {
    rsx! {
        span {
            class: "loading inline-flex align-middle leading-none font-bold p-[2px]",
            class: "before:content-['♫'] before:block before:animate-spin",
        }
    }
}
