use std::sync::Arc;

use lol_html::{element, html_content::ContentType, Settings};

use crate::{
    error::Result, proxy::util::encode_url, rewriting::rewriter::Rewriter, state::SharedState,
};

pub struct HtmlRewriter {
    state: Arc<SharedState>,
}

impl HtmlRewriter {
    pub fn new(state: Arc<SharedState>) -> Self {
        Self { state }
    }
}

impl Rewriter for HtmlRewriter {
    fn rewrite(&self, input: Vec<u8>) -> Result<Vec<u8>> {
        let mut output = vec![];
        let mut rewriter = lol_html::HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![
                    // Inject console.log script in head
                    element!("head", |el| {
                        el.append(
                            &format!(
                                r#"<script type="text/javascript">{}</script>"#,
                                include_str!("../patches.js")
                            ),
                            ContentType::Html,
                        );

                        Ok(())
                    }),
                    element!("[href]", |el| {
                        let href = el.get_attribute("href").unwrap();

                        el.set_attribute("href", &encode_url(&self.state.config, &href))
                            .unwrap();

                        Ok(())
                    }),
                    element!("[src]", |el| {
                        let src = el.get_attribute("src").unwrap();

                        el.set_attribute("src", &encode_url(&self.state.config, &src))
                            .unwrap();

                        Ok(())
                    }),
                    element!("[poster]", |el| {
                        let poster = el.get_attribute("poster").unwrap();

                        el.set_attribute("poster", &encode_url(&self.state.config, &poster))
                            .unwrap();

                        Ok(())
                    }),
                ],

                ..Settings::default()
            },
            |c: &[u8]| output.extend_from_slice(c),
        );

        rewriter.write(&input)?;

        rewriter.end()?;

        Ok(output)
    }
}
