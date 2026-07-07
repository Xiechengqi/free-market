use std::{collections::HashMap, sync::Arc};

use crate::payment::{
    provider::PaymentProvider,
    providers::{
        AlipayProvider, BepusdtProvider, DujiaoPayProvider, EpayProvider, EpusdtProvider,
        NoopProvider, OkpayProvider, PaypalProvider, StripeProvider, TokenPayProvider,
        WechatPayProvider,
    },
};

#[derive(Clone)]
pub struct PaymentRegistry {
    providers: Arc<HashMap<String, Arc<dyn PaymentProvider>>>,
}

impl PaymentRegistry {
    pub fn default_registry() -> Self {
        let mut providers: HashMap<String, Arc<dyn PaymentProvider>> = HashMap::new();
        providers.insert("noop:".to_string(), Arc::new(NoopProvider));
        providers.insert("epay:".to_string(), Arc::new(EpayProvider));
        providers.insert("yipay:".to_string(), Arc::new(EpayProvider));
        providers.insert("tokenpay:".to_string(), Arc::new(TokenPayProvider));
        providers.insert("epusdt:".to_string(), Arc::new(EpusdtProvider));
        providers.insert("bepusdt:".to_string(), Arc::new(BepusdtProvider));
        providers.insert("dujiaopay:".to_string(), Arc::new(DujiaoPayProvider));
        providers.insert("okpay:".to_string(), Arc::new(OkpayProvider));
        providers.insert("official:stripe".to_string(), Arc::new(StripeProvider));
        providers.insert("official:paypal".to_string(), Arc::new(PaypalProvider));
        providers.insert("official:alipay".to_string(), Arc::new(AlipayProvider));
        providers.insert("official:wechat".to_string(), Arc::new(WechatPayProvider));
        providers.insert("official:wxpay".to_string(), Arc::new(WechatPayProvider));
        Self {
            providers: Arc::new(providers),
        }
    }

    pub fn lookup(
        &self,
        provider_type: &str,
        channel_type: &str,
    ) -> Option<Arc<dyn PaymentProvider>> {
        let exact = format!("{}:{}", normalize(provider_type), normalize(channel_type));
        self.providers.get(&exact).cloned().or_else(|| {
            self.providers
                .get(&format!("{}:", normalize(provider_type)))
                .cloned()
        })
    }
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}
