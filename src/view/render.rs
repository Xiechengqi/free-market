use minijinja::{Environment, context};
use serde::Serialize;

use crate::{config::SiteConfig, error::AppError};

pub struct ViewRenderer {
    env: Environment<'static>,
    csrf_token: String,
    admin_prefix: String,
}

impl ViewRenderer {
    #[allow(dead_code)]
    pub fn new(csrf_token: String) -> anyhow::Result<Self> {
        Self::with_admin_prefix(csrf_token, "/admin".to_string())
    }

    pub fn with_admin_prefix(csrf_token: String, admin_prefix: String) -> anyhow::Result<Self> {
        let mut env = Environment::new();
        env.add_template(
            "luna/home.html",
            include_str!("../../templates/luna/home.html"),
        )?;
        env.add_template(
            "luna/buy.html",
            include_str!("../../templates/luna/buy.html"),
        )?;
        env.add_template(
            "luna/bill.html",
            include_str!("../../templates/luna/bill.html"),
        )?;
        env.add_template(
            "luna/order.html",
            include_str!("../../templates/luna/order.html"),
        )?;
        env.add_template(
            "luna/orders.html",
            include_str!("../../templates/luna/orders.html"),
        )?;
        env.add_template(
            "luna/search.html",
            include_str!("../../templates/luna/search.html"),
        )?;
        env.add_template(
            "luna/pay.html",
            include_str!("../../templates/luna/pay.html"),
        )?;
        env.add_template(
            "unicorn/home.html",
            include_str!("../../templates/unicorn/home.html"),
        )?;
        env.add_template(
            "unicorn/buy.html",
            include_str!("../../templates/unicorn/buy.html"),
        )?;
        env.add_template(
            "unicorn/bill.html",
            include_str!("../../templates/unicorn/bill.html"),
        )?;
        env.add_template(
            "unicorn/order.html",
            include_str!("../../templates/unicorn/order.html"),
        )?;
        env.add_template(
            "unicorn/orders.html",
            include_str!("../../templates/unicorn/orders.html"),
        )?;
        env.add_template(
            "unicorn/search.html",
            include_str!("../../templates/unicorn/search.html"),
        )?;
        env.add_template(
            "unicorn/pay.html",
            include_str!("../../templates/unicorn/pay.html"),
        )?;
        env.add_template(
            "hyper/home.html",
            include_str!("../../templates/hyper/home.html"),
        )?;
        env.add_template(
            "hyper/buy.html",
            include_str!("../../templates/hyper/buy.html"),
        )?;
        env.add_template(
            "hyper/bill.html",
            include_str!("../../templates/hyper/bill.html"),
        )?;
        env.add_template(
            "hyper/order.html",
            include_str!("../../templates/hyper/order.html"),
        )?;
        env.add_template(
            "hyper/orders.html",
            include_str!("../../templates/hyper/orders.html"),
        )?;
        env.add_template(
            "hyper/search.html",
            include_str!("../../templates/hyper/search.html"),
        )?;
        env.add_template(
            "hyper/pay.html",
            include_str!("../../templates/hyper/pay.html"),
        )?;
        Ok(Self {
            env,
            csrf_token,
            admin_prefix,
        })
    }

    pub fn render<T: Serialize>(
        &self,
        name: &str,
        site: &SiteConfig,
        data: T,
    ) -> Result<String, AppError> {
        let tmpl = self
            .env
            .get_template(name)
            .map_err(|err| AppError::Anyhow(err.into()))?;
        let rendered = tmpl
            .render(context! {
                site => site,
                data => data,
                csrf => self.csrf_token,
                admin_prefix => self.admin_prefix,
            })
            .map_err(|err| AppError::Anyhow(err.into()))?;
        if name.starts_with("admin/") {
            return Ok(rendered);
        }
        Ok(inject_site_scripts(rendered, site))
    }
}

fn inject_site_scripts(mut html: String, site: &SiteConfig) -> String {
    let mut snippets = String::new();
    if site.is_open_anti_red {
        snippets.push_str(ANTI_RED_SNIPPET);
    }
    if site.is_open_google_translate {
        snippets.push_str(GOOGLE_TRANSLATE_SNIPPET);
    }
    if snippets.is_empty() {
        return html;
    }
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, &snippets);
    } else {
        html.push_str(&snippets);
    }
    html
}

const ANTI_RED_SNIPPET: &str = r#"
<script>(function(){var ua=navigator.userAgent||'';if(/MicroMessenger|QQ\/[0-9]/i.test(ua)){var b=document.createElement('div');b.style.cssText='position:fixed;top:0;left:0;right:0;padding:14px 16px;background:#fff3cd;color:#8a6d3b;border-bottom:1px solid #faebcc;font-size:14px;z-index:99999;text-align:center;';b.innerHTML='检测到您在微信/QQ 内置浏览器，部分支付可能受限，建议点击右上角“在浏览器中打开”。';document.body.appendChild(b);}})();</script>
"#;

const GOOGLE_TRANSLATE_SNIPPET: &str = r#"
<div id="google_translate_element" style="position:fixed;right:12px;bottom:12px;z-index:99998;"></div>
<script>function googleTranslateElementInit(){new google.translate.TranslateElement({pageLanguage:'zh-CN',autoDisplay:false},'google_translate_element');}</script>
<script src="https://translate.google.com/translate_a/element.js?cb=googleTranslateElementInit" async></script>
"#;

pub fn frontend_template(site: &SiteConfig, page: &str) -> String {
    let theme = match site.theme.as_str() {
        "unicorn" => "unicorn",
        "hyper" => "hyper",
        _ => "luna",
    };
    format!("{theme}/{page}")
}
