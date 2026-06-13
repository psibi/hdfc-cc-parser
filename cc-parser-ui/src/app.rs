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
    let (encrypted, set_encrypted) = signal(false);
    let (password, set_password) = signal(String::new());
    let (pending_bytes, set_pending_bytes) = signal(None::<Vec<u8>>);

    let csv_string = move || {
        let txns = transactions.get();
        if txns.is_empty() {
            return String::new();
        }
        let mut csv = String::from("Date,Description,Amount\n");
        for t in &txns {
            csv.push_str(&format!("{},{},{:.2}\n", t.date, t.description, t.amount));
        }
        csv
    };

    let on_file_change = move |ev: leptos::ev::Event| {
        let Some(target) = ev.target() else {
            return;
        };
        let target = target.unchecked_into::<HtmlInputElement>();
        let files = target.files();
        if let Some(files) = files {
            if let Some(file) = files.get(0) {
                let name = file.name();
                set_file_name.set(name.clone());
                set_error.set(None);
                set_processing.set(true);

                let _name_clone = name.clone();
                leptos::task::spawn_local(async move {
                    let promise = file.array_buffer();
                    let result = JsFuture::from(promise).await;
                    let Ok(array_buffer) = result else {
                        set_error.set(Some("Failed to read file".into()));
                        set_processing.set(false);
                        return;
                    };
                    let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
                    if hdfc_cc_parser::is_pdf_encrypted(&bytes) {
                        set_encrypted.set(true);
                        set_pending_bytes.set(Some(bytes));
                        set_processing.set(false);
                        return;
                    }

                    set_encrypted.set(false);
                    set_pending_bytes.set(None);

                    match hdfc_cc_parser::parse_pdf_bytes(&bytes) {
                        Ok(txns) => {
                            set_parsed_count.set(txns.len());
                            set_transactions.set(txns);
                            set_processing.set(false);
                        }
                        Err(e) => {
                            set_error.set(Some(format!("Parsing error: {e}")));
                            set_processing.set(false);
                        }
                    }
                });
            }
        }
    };

    let submit_password = move |ev: leptos::ev::MouseEvent| {
        ev.prevent_default();
        let pwd = password.get();
        if pwd.is_empty() {
            return;
        }
        if let Some(bytes) = pending_bytes.get() {
            set_encrypted.set(false);
            set_processing.set(true);
            set_error.set(None);
            let pwd = pwd.clone();
            leptos::task::spawn_local(async move {
                match hdfc_cc_parser::parse_pdf_bytes_with_password(&bytes, &pwd) {
                    Ok(txns) => {
                        set_parsed_count.set(txns.len());
                        set_transactions.set(txns);
                        set_processing.set(false);
                    }
                    Err(e) => {
                        let msg = format!("{e}");
                        if msg.contains("InvalidPassword") || msg.contains("password") {
                            set_error.set(Some("Incorrect password. Try again.".into()));
                            set_encrypted.set(true);
                        } else {
                            set_error.set(Some(format!("Parsing error: {e}")));
                        }
                        set_processing.set(false);
                    }
                }
            });
        }
    };

    let cancel_password = move |ev: leptos::ev::MouseEvent| {
        ev.prevent_default();
        set_encrypted.set(false);
        set_pending_bytes.set(None);
        set_password.set(String::new());
    };

    let on_password_keydown = move |ev: leptos::ev::KeyboardEvent| {
        if ev.key() == "Enter" {
            let pwd = password.get();
            if pwd.is_empty() {
                return;
            }
            if let Some(bytes) = pending_bytes.get() {
                set_encrypted.set(false);
                set_processing.set(true);
                set_error.set(None);
                let pwd = pwd.clone();
                leptos::task::spawn_local(async move {
                    match hdfc_cc_parser::parse_pdf_bytes_with_password(&bytes, &pwd) {
                        Ok(txns) => {
                            set_parsed_count.set(txns.len());
                            set_transactions.set(txns);
                            set_processing.set(false);
                        }
                        Err(e) => {
                            let msg = format!("{e}");
                            if msg.contains("InvalidPassword") || msg.contains("password") {
                                set_error.set(Some("Incorrect password. Try again.".into()));
                                set_encrypted.set(true);
                            } else {
                                set_error.set(Some(format!("Parsing error: {e}")));
                            }
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
            match Blob::new_with_str_sequence_and_options(&js_sys::Array::of1(&csv.into()), &props)
            {
                Ok(b) => b,
                Err(e) => {
                    set_error.set(Some(format!("Failed to create blob: {e:?}")));
                    return;
                }
            };

        let url = match Url::create_object_url_with_blob(&blob) {
            Ok(u) => u,
            Err(e) => {
                set_error.set(Some(format!("Failed to create URL: {e:?}")));
                return;
            }
        };
        let Some(window) = web_sys::window() else {
            set_error.set(Some("Window object unavailable".into()));
            return;
        };
        let Some(document) = window.document() else {
            set_error.set(Some("Document object unavailable".into()));
            return;
        };
        let a = match document.create_element("a") {
            Ok(el) => el,
            Err(e) => {
                set_error.set(Some(format!("Failed to create element: {e:?}")));
                return;
            }
        };
        let a = a.unchecked_into::<web_sys::HtmlAnchorElement>();
        a.set_href(&url);
        let out_name = if file_name.get().ends_with(".pdf") {
            file_name.get().replace(".pdf", ".csv")
        } else {
            format!("{}.csv", file_name.get())
        };
        a.set_download(&out_name);
        a.click();
        let _ = Url::revoke_object_url(&url);
    };

    view! {
        <div class="container">
            <h1>"HDFC Infinia CC Parser"</h1>
            <p>"Upload a HDFC Infinia credit card PDF statement to extract transactions as CSV."</p>

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

            {move || encrypted.get().then(|| view! {
                <div class="password-prompt">
                    <p>"This PDF is password protected. Enter the password to decrypt it."</p>
                    <input
                        type="password"
                        placeholder="PDF password"
                        on:input=move |ev| {
                            set_password.set(event_target_value(&ev));
                        }
                        on:keydown=on_password_keydown
                    />
                    <button class="decrypt-btn" on:click=submit_password>
                        "Decrypt"
                    </button>
                    <button class="cancel-btn" on:click=cancel_password>
                        "Cancel"
                    </button>
                </div>
            })}

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
            <div class="privacy-note">
                "🔒 No data leaves your device — all parsing happens client-side via WebAssembly."
            </div>
            <footer>
                <div class="footer-links">
                    <a href="http://psibi.in/" target="_blank" rel="noopener noreferrer">"🏠 Author"</a>
                    <span class="sep">"·"</span>
                    <a href="https://github.com/psibi/hdfc-cc-parser" target="_blank" rel="noopener noreferrer">
                        <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor" style="vertical-align:middle;margin-right:0.25em;">
                            <path fill-rule="evenodd" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"/>
                        </svg>
                        "Source"
                    </a>
                </div>
            </footer>
        </div>
    }
}
