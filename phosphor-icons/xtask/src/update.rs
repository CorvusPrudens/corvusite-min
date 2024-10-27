use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::{fs, process};

fn extract_categories(input: &str) -> (HashMap<String, Vec<String>>, BTreeMap<String, ()>) {
    let mut icon_categories: HashMap<String, Vec<String>> = HashMap::new();
    let mut categories_set: BTreeMap<String, ()> = BTreeMap::new();

    let re = Regex::new(r#"(?m)^\s*name:\s*"(.+)",\n.*\n\s*categories:\s*\[([^]]+)\]"#).unwrap();

    for cap in re.captures_iter(input) {
        let name = cap[1].to_string();
        let has_categories = cap[2]
            .split(',')
            .filter(|category| !category.trim().is_empty())
            .map(|category| {
                let value = category
                    .trim()
                    .split('.')
                    .nth(1)
                    .unwrap()
                    .to_lowercase()
                    .to_string();
                categories_set.insert(value.clone(), ());
                value
            })
            .collect::<Vec<String>>();

        icon_categories.insert(name, has_categories);
    }
    // Insert the Uncategorized category for icons that are not in the TS export file
    categories_set.insert("uncategorized".to_string(), ());
    (icon_categories, categories_set)
}

const OUTPUT_DIR: &str = "icons";
const ASSETS_DIR: &str = "core/assets";
const TYPESCRIPT_EXPORT_FILE: &str = "core/src/icons.ts";

pub fn run() {
    let svg_tag_regex = Regex::new(r"<svg.*?>").unwrap();
    let svg_closing_tag_regex = Regex::new(r"</svg>").unwrap();

    // Extract the categories from the typescript export file
    let (icon_categories, categories_set) =
        extract_categories(fs::read_to_string(TYPESCRIPT_EXPORT_FILE).unwrap().as_str());

    let uncategorized = vec!["uncategorized".into()];

    // Clean up the icons folder
    let _ = fs::remove_dir_all(OUTPUT_DIR);
    fs::write("src/lib.rs", "").unwrap();
    fs::create_dir(OUTPUT_DIR).unwrap();

    // Get a list of all the icon weights
    let mut weights: Vec<_> = fs::read_dir(ASSETS_DIR)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();

    // Sort the weights so their ordering is stable.
    weights.sort_unstable();

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
    // stable order. This should improve `src/mod.rs` diffs.
    file_names.sort_unstable();

    for file_name in file_names {
        let icon_name = file_name.strip_suffix(".svg").unwrap().to_string();

        //derive the feature set string for this icon from its mappings.
        //If we haven't been able to match the icon's category, assign in to 'Uncategorized'
        let features = icon_categories.get(&icon_name).unwrap_or(&uncategorized);

        let icon_weights = weights.iter().map(|weight| {
            let file_name = if weight == "regular" {
                format!("{icon_name}.svg")
            } else {
                format!("{icon_name}-{weight}.svg")
            };
            let svg = fs::read_to_string(format!("{ASSETS_DIR}/{weight}/{file_name}")).unwrap();
            let svg = svg_tag_regex.replace(&svg, "");
            let svg = svg_closing_tag_regex.replace(&svg, "");
            (weight.to_string(), svg.to_string())
        });

        for (weight_name, data) in icon_weights {
            let component_name = format!(
                "{}{}",
                icon_name.to_case(Case::Pascal),
                weight_name.to_case(Case::Pascal)
            );

            fs::write(
                format!(
                    "{OUTPUT_DIR}/{}-{weight_name}.mod.html",
                    icon_name.to_case(Case::Kebab)
                ),
                format!(
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
                ),
            )
            .unwrap();
        }
    }
}
