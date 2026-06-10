use hdfc_cc_parser::Transaction;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, BlobPropertyBag, HtmlInputElement, Url};

#[component]
pub fn App() -> impl IntoView {
    let (transactions, set_transactions) = signal(Vec::<Transaction>::new());
    let (error, set_error) = signal(None::<String>);
    let (file_name, set_file_name) = signal(String::new());
    let (processing, set_processing) = signal(false);
    let (parsed_count, set_parsed_count) = signal(0usize);

    let csv_string = move || {
        let txns = transactions.get();
        if txns.is_empty() {
            return String::new();
        }
        let mut csv = String::from("Date,Description,Amount\n");
        for t in &txns {
            csv.push_str(&format!(
                "{},{},{:.2}\n",
                t.date, t.description, t.amount
            ));
        }
        csv
    };

    let on_file_change = move |ev: leptos::ev::Event| {
                    web_sys::console::log_1(&"on_file_change fired".into());
        let target = ev
            .target()
            .unwrap()
            .unchecked_into::<HtmlInputElement>();
        let files = target.files();
        if let Some(files) = files {
            if let Some(file) = files.get(0) {
                let name = file.name();
                web_sys::console::log_1(&format!("file: {name}").into());
                set_file_name.set(name.clone());
                set_error.set(None);
                set_processing.set(true);

                leptos::task::spawn_local(async move {
                    let promise = file.array_buffer();
                    let result = JsFuture::from(promise).await;
                    let array_buffer = result.unwrap();
                    let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
                    web_sys::console::log_1(&format!("read {} bytes", bytes.len()).into());

                    match hdfc_cc_parser::extract_lines_from_pdf(&bytes) {
                        Ok(lines) => {
                            web_sys::console::log_1(&format!("extracted {} lines", lines.len()).into());
                            let show = lines.len().min(40);
                            for (i, line) in lines.iter().enumerate().take(show) {
                                web_sys::console::log_1(&format!("  line[{i}]: [{line}]").into());
                            }
                            let date_re = regex::Regex::new(r"(\d{2}/\d{2}/\d{4}\s*\|\s*\d{2}:\d{2})").unwrap();
                            let date_matches: Vec<_> = lines.iter().enumerate().filter(|(_, l)| date_re.is_match(l)).collect();
                            web_sys::console::log_1(&format!("lines with date pattern (4-digit yr): {}", date_matches.len()).into());

                            let date_re2 = regex::Regex::new(r"\d{2}/\d{2}/\d{2}").unwrap();
                            let any_date: Vec<_> = lines.iter().enumerate().filter(|(_, l)| date_re2.is_match(l)).collect();
                            web_sys::console::log_1(&format!("lines with any dd/mm/yy: {}", any_date.len()).into());
                            for (i, line) in any_date.iter().take(5) {
                                web_sys::console::log_1(&format!("  anydate_line[{i}]: [{line}]").into());
                            }

                            let ref_re = regex::Regex::new(r"Ref#").unwrap();
                            let ref_lines: Vec<_> = lines.iter().enumerate().filter(|(_, l)| ref_re.is_match(l)).collect();
                            web_sys::console::log_1(&format!("lines with Ref#: {}", ref_lines.len()).into());
                        }
                        Err(e) => {
                            web_sys::console::log_1(&format!("extract_lines error: {e}").into());
                        }
                    }

                    match hdfc_cc_parser::parse_pdf_bytes(&bytes) {
                        Ok(txns) => {
                            set_parsed_count.set(txns.len());
                            web_sys::console::log_1(&format!("parsed {} transactions", txns.len()).into());
                            set_transactions.set(txns);
                            set_processing.set(false);
                        }
                        Err(e) => {
                            web_sys::console::log_1(&format!("parse error: {e}").into());
                            set_error.set(Some(format!("Parsing error: {e}")));
                            set_processing.set(false);
                        }
                    }
                });
            }
        }
    };

    let export_csv = move |_| {
        let csv = csv_string();
        if csv.is_empty() {
            return;
        }

        let props = BlobPropertyBag::new();
        props.set_type("text/csv");
        let blob =
            Blob::new_with_str_sequence_and_options(&js_sys::Array::of1(&csv.into()), &props)
                .unwrap();

        let url = Url::create_object_url_with_blob(&blob).unwrap();
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let a = document
            .create_element("a")
            .unwrap()
            .unchecked_into::<web_sys::HtmlAnchorElement>();
        a.set_href(&url);
        let out_name = if file_name.get().ends_with(".pdf") {
            file_name.get().replace(".pdf", ".csv")
        } else {
            format!("{}.csv", file_name.get())
        };
        a.set_download(&out_name);
        a.click();
        Url::revoke_object_url(&url).unwrap();
    };

    view! {
        <div class="container">
            <h1>"HDFC CC Parser"</h1>
            <p>"Upload a HDFC credit card PDF statement to extract transactions as CSV."</p>

            <label class="file-label" for="pdf-upload">
                "Choose PDF file"
            </label>
            <input
                id="pdf-upload"
                type="file"
                accept=".pdf"
                on:change=on_file_change
            />

            {move || processing.get().then(|| view! { <p class="status">"Processing..."</p> })}

            {move || error.get().map(|e| view! { <p class="error">{e}</p> })}

            {move || {
                let count = parsed_count.get();
                let processing = processing.get();
                let has_file = !file_name.get().is_empty();
                let csv = csv_string();
                if count > 0 {
                    view! {
                        <div class="result-bar">
                            <span>{format!("{count} transactions found")}</span>
                            <button class="export-btn" on:click=export_csv>
                                "⬇ Download CSV"
                            </button>
                        </div>
                        <div class="csv-preview">
                            <h3>"CSV Preview"</h3>
                            <pre>{csv}</pre>
                        </div>
                    }.into_any()
                } else if has_file && !processing {
                    view! {
                        <div class="result-bar">
                            <span>"0 transactions found"</span>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

            <div class="table-wrap">
            {move || {
                let txns = transactions.get();
                if txns.is_empty() {
                    view! { <div></div> }.into_any()
                } else {
                    view! {
                        <table>
                            <thead>
                                <tr>
                                    <th>"Date"</th>
                                    <th>"Description"</th>
                                    <th>"Amount (₹)"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {txns.iter().map(|t| view! {
                                    <tr>
                                        <td>{t.date.clone()}</td>
                                        <td class="desc">{t.description.clone()}</td>
                                        <td class={if t.amount < 0.0 { "debit" } else { "credit" }}>
                                            {format!("{:.2}", t.amount)}
                                        </td>
                                    </tr>
                                }).collect::<Vec<_>>()}
                            </tbody>
                        </table>
                    }.into_any()
                }
            }}
            </div>
        </div>
    }
}
