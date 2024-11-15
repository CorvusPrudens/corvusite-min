use convert_case::{Case, Casing};
use quote::quote;
use regex::Regex;
use std::path::Path;
use std::{env, fs, rc::Rc};

const ASSETS_DIR: &str = "phosphor-icons/core/assets";

pub fn run() -> impl Iterator<Item = (String, String)> {
    let svg_tag_regex: &_ = Box::leak(Box::new(Regex::new(r"<svg.*?>").unwrap()));
    let svg_closing_tag_regex: &_ = Box::leak(Box::new(Regex::new(r"</svg>").unwrap()));

    // Get a list of all the icon weights
    let mut weights: Vec<_> = fs::read_dir(ASSETS_DIR)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();

    // Sort the weights so their ordering is stable.
    weights.sort_unstable();

    let weights: &_ = Vec::leak(weights);

    let regular_icons = fs::read_dir(format!("{ASSETS_DIR}/regular")).unwrap();

    let mut file_names: Vec<_> = regular_icons
        .into_iter()
        .filter_map(|e| {
            let entry = e.unwrap();
            if entry.path().is_file() {
                Some(entry.file_name().into_string().unwrap())
            } else {
                None
            }
        })
        .collect();

    // We'll also sort the file names so each generation run has a
    // stable order.
    file_names.sort_unstable();

    file_names.into_iter().flat_map(move |file_name| {
        let icon_name: Rc<str> = Rc::from(file_name.strip_suffix(".svg").unwrap());

        let icon_weights = weights.iter().map({
            let icon_name = Rc::clone(&icon_name);

            move |weight| {
                let file_name = if weight == "regular" {
                    format!("{icon_name}.svg")
                } else {
                    format!("{icon_name}-{weight}.svg")
                };
                let svg = fs::read_to_string(format!("{ASSETS_DIR}/{weight}/{file_name}")).unwrap();
                let svg = svg_tag_regex.replace(&svg, "");
                let svg = svg_closing_tag_regex.replace(&svg, "");
                (weight.to_owned(), svg.to_string())
            }
        });

        icon_weights.map(move |(weight_name, data)| {
            let component_name = format!(
                "{}{}",
                icon_name.as_ref().to_case(Case::Pascal),
                weight_name.to_case(Case::Pascal)
            );

            let body = format!(
                r#"
                    <{component_name} size="24px" fill class>
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            width="size"
                            height="size"
                            fill="fill"
                            viewBox="0 0 256 256"
                            class="class"
                        >
                            {data}
                        </svg>
                    </{component_name}>
                "#
            );

            (component_name, body)
        })
    })
}

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("icons.rs");

    let components = run().map(|(name, data)| {
        quote! {
            (#name, #data)
        }
    });

    let output = quote! {
        pub fn icons<S>() -> LazyComponents<'static, S>
        where S: std::hash::BuildHasher + Default
        {
            const ICONS: &[(&str, &str)] = &[
                #(#components),*
            ];

            LazyComponents(ICONS.iter().map(|(name, raw)| (*name, LazyComponent::new(raw))).collect())
        }
    };

    std::fs::write(dest_path, output.to_string()).unwrap();

    println!("cargo::rerun-if-changed=phosphor-icons/core");
    println!("cargo::rerun-if-changed=build.rs");
}
