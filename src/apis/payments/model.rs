use crate::{apis::auth::Token, pollable::IsInTerminalState, Error, Pollable, TrueLayerClient};
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreatePaymentRequest {
    pub amount_in_minor: u64,
    pub currency: Currency,
    pub payment_method: PaymentMethod,
    pub user: CreatePaymentUserRequest,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum CreatePaymentUserRequest {
    ExistingUser {
        id: String,
    },
    NewUser {
        name: Option<String>,
        email: Option<String>,
        phone: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreatePaymentResponse {
    pub id: String,
    pub resource_token: Token,
    pub user: CreatePaymentUserResponse,
}

#[async_trait]
impl Pollable for CreatePaymentResponse {
    type Output = Payment;

    async fn poll_once(&self, tl: &TrueLayerClient) -> Result<Self::Output, Error> {
        tl.payments
            .get_by_id(&self.id)
            .await
            .transpose()
            .unwrap_or_else(|| Err(Error::Other(anyhow!("Payment returned 404 while polling"))))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreatePaymentUserResponse {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Payment {
    pub id: String,
    pub amount_in_minor: u64,
    pub currency: Currency,
    pub user: User,
    pub payment_method: PaymentMethod,
    pub created_at: DateTime<Utc>,
    pub metadata: Option<HashMap<String, String>>,
    #[serde(flatten)]
    pub status: PaymentStatus,
}

#[async_trait]
impl Pollable for Payment {
    type Output = Payment;

    async fn poll_once(&self, tl: &TrueLayerClient) -> Result<Self::Output, Error> {
        tl.payments
            .get_by_id(&self.id)
            .await
            .transpose()
            .unwrap_or_else(|| Err(Error::Other(anyhow!("Payment returned 404 while polling"))))
    }
}

impl IsInTerminalState for Payment {
    /// A payment is considered to be in a terminal state if it is `Executed`, `Settled` or `Failed`.
    fn is_in_terminal_state(&self) -> bool {
        matches!(
            self.status,
            PaymentStatus::Executed { .. }
                | PaymentStatus::Settled { .. }
                | PaymentStatus::Failed { .. }
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PaymentStatus {
    AuthorizationRequired,
    Authorizing {
        authorization_flow: AuthorizationFlow,
    },
    Authorized {
        authorization_flow: Option<AuthorizationFlow>,
    },
    Executed {
        executed_at: DateTime<Utc>,
        authorization_flow: Option<AuthorizationFlow>,
        settlement_risk: Option<SettlementRisk>,
    },
    Settled {
        payment_source: PaymentSource,
        executed_at: DateTime<Utc>,
        settled_at: DateTime<Utc>,
        authorization_flow: Option<AuthorizationFlow>,
        settlement_risk: Option<SettlementRisk>,
    },
    Failed {
        failed_at: DateTime<Utc>,
        failure_stage: FailureStage,
        failure_reason: String,
        authorization_flow: Option<AuthorizationFlow>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    Gbp,
    Eur,
}

impl Display for Currency {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Currency::Gbp => write!(f, "GBP"),
            Currency::Eur => write!(f, "EUR"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FailureStage {
    AuthorizationRequired,
    Authorizing,
    Authorized,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct PaymentSource {
    pub id: String,
    pub user_id: Option<String>,
    #[serde(default)]
    pub account_identifiers: Vec<AccountIdentifier>,
    pub account_holder_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaymentMethod {
    BankTransfer {
        provider_selection: ProviderSelection,
        beneficiary: Beneficiary,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Beneficiary {
    MerchantAccount {
        merchant_account_id: String,
        account_holder_name: Option<String>,
    },
    ExternalAccount {
        account_holder_name: String,
        account_identifier: AccountIdentifier,
        reference: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AccountIdentifier {
    SortCodeAccountNumber {
        sort_code: String,
        account_number: String,
    },
    Iban {
        iban: String,
    },
    Bban {
        bban: String,
    },
    Nrb {
        nrb: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SettlementRisk {
    pub category: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderSelection {
    UserSelected {
        filter: Option<ProviderFilter>,
        preferred_scheme_ids: Option<Vec<String>>,
    },
    Preselected {
        provider_id: String,
        scheme_id: String,
        remitter: Option<Remitter>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Remitter {
    pub account_holder_name: Option<String>,
    pub account_identifier: Option<AccountIdentifier>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ProviderFilter {
    pub countries: Option<Vec<CountryCode>>,
    pub release_channel: Option<ReleaseChannel>,
    pub customer_segments: Option<Vec<CustomerSegment>>,
    pub provider_ids: Option<Vec<String>>,
    pub excludes: Option<ProviderFilterExcludes>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum CountryCode {
    DE,
    ES,
    FR,
    GB,
    IE,
    IT,
    LT,
    NL,
    PL,
    PT,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    GeneralAvailability,
    PublicBeta,
    PrivateBeta,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CustomerSegment {
    Retail,
    Business,
    Corporate,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ProviderFilterExcludes {
    pub provider_ids: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AuthorizationFlow {
    pub actions: Option<AuthorizationFlowActions>,
    pub configuration: Option<AuthorizationFlowConfiguration>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AuthorizationFlowActions {
    pub next: AuthorizationFlowNextAction,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthorizationFlowNextAction {
    ProviderSelection {
        providers: Vec<Provider>,
    },
    Redirect {
        uri: String,
        metadata: Option<RedirectActionMetadata>,
    },
    Form {
        inputs: Vec<AdditionalInput>,
    },
    Wait,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Provider {
    pub id: String,
    pub display_name: Option<String>,
    pub icon_uri: Option<String>,
    pub logo_uri: Option<String>,
    pub bg_color: Option<String>,
    pub country_code: Option<CountryCode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RedirectActionMetadata {
    Provider(Provider),
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdditionalInput {
    Text {
        id: String,
        mandatory: bool,
        display_text: AdditionalInputDisplayText,
        description: Option<AdditionalInputDisplayText>,
        format: AdditionalInputFormat,
        sensitive: bool,
        min_length: i32,
        max_length: i32,
        regexes: Vec<AdditionalInputRegex>,
    },
    Select {
        id: String,
        mandatory: bool,
        display_text: AdditionalInputDisplayText,
        description: Option<AdditionalInputDisplayText>,
        options: Vec<AdditionalInputOption>,
    },
    TextWithImage {
        id: String,
        mandatory: bool,
        display_text: AdditionalInputDisplayText,
        description: Option<AdditionalInputDisplayText>,
        format: AdditionalInputFormat,
        sensitive: bool,
        min_length: i32,
        max_length: i32,
        regexes: Vec<AdditionalInputRegex>,
        image: AdditionalInputImage,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AdditionalInputDisplayText {
    pub key: String,
    pub default: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdditionalInputFormat {
    AccountNumber,
    Alphabetical,
    Alphanumerical,
    Any,
    Email,
    Iban,
    Numerical,
    SortCode,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AdditionalInputRegex {
    pub regex: String,
    pub message: AdditionalInputDisplayText,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AdditionalInputOption {
    pub id: String,
    pub display_text: AdditionalInputDisplayText,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdditionalInputImage {
    Uri { uri: String },
    Base64 { data: String, media_type: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct AuthorizationFlowConfiguration {
    pub provider_selection: Option<ProviderSelectionSupported>,
    pub redirect: Option<RedirectSupported>,
    pub form: Option<FormSupported>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ProviderSelectionSupported {}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct RedirectSupported {
    pub return_uri: String,
    pub direct_return_uri: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct FormSupported {
    pub input_types: Vec<AdditionalInputType>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AdditionalInputType {
    Text,
    Select,
    TextWithImage,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct User {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct StartAuthorizationFlowRequest {
    pub provider_selection: Option<ProviderSelectionSupported>,
    pub redirect: Option<RedirectSupported>,
    pub form: Option<FormSupported>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct StartAuthorizationFlowResponse {
    pub authorization_flow: Option<AuthorizationFlow>,
    #[serde(flatten)]
    pub status: AuthorizationFlowResponseStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitProviderSelectionActionRequest {
    pub provider_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitProviderSelectionActionResponse {
    pub authorization_flow: Option<AuthorizationFlow>,
    #[serde(flatten)]
    pub status: AuthorizationFlowResponseStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitFormActionRequest {
    pub inputs: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitFormActionResponse {
    pub authorization_flow: Option<AuthorizationFlow>,
    #[serde(flatten)]
    pub status: AuthorizationFlowResponseStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuthorizationFlowResponseStatus {
    Authorizing,
    Failed {
        failure_stage: FailureStage,
        failure_reason: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitProviderReturnParametersRequest {
    pub query: String,
    pub fragment: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct SubmitProviderReturnParametersResponse {
    pub resource: SubmitProviderReturnParametersResponseResource,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubmitProviderReturnParametersResponseResource {
    Payment { payment_id: String },
}
