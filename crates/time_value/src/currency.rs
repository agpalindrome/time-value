//! [`Currency`] — an ISO 4217 currency, carried as a runtime value on
//! [`Money`](crate::Money).
//!
//! Per `docs/adr/0034-money-and-currency.md`, currency is *dynamic* data (it
//! arrives at runtime from user input, a config, or an exchange feed), so it is a
//! runtime value rather than a compile-time type tag. The enum ships the full
//! ISO 4217 active set — fiat, the precious-metal codes, the fund and unit-of-
//! account codes, and the reserved [`Xxx`](Currency::Xxx)/[`Xts`](Currency::Xts).
//! The variants and their metadata tables were generated from the ISO 4217
//! published list (alphabetic + numeric codes and minor-unit exponents). Each
//! metadata table is an *exhaustive `match`* over the variants, so the compiler
//! guarantees every variant carries a code, a numeric code and a minor-unit
//! exponent; when ISO amends the list, adding the variant makes those matches stop
//! compiling until they cover it.

use core::fmt;

use crate::TvmError;

/// An ISO 4217 currency.
///
/// A plain `Copy` enum: equality, hashing and ordering are trivial, and its
/// metadata — the [`code`](Self::code), [`numeric`](Self::numeric) code, and
/// [`minor_unit_exponent`](Self::minor_unit_exponent) — is exposed by `const`
/// methods backed by exhaustive match tables, so it is curated and total.
///
/// [`Xxx`](Self::Xxx) (ISO 4217 "no currency") is the **currency-agnostic**
/// amount and the identity on the currency axis: combining `Xxx` with a currency
/// `C` yields `C`, and two *distinct* non-`Xxx` currencies are a
/// [`TvmError::CurrencyMismatch`](crate::TvmError::CurrencyMismatch). Pure-number
/// TVM is therefore all `Xxx` and computes exactly as an untagged core would.
///
/// As a *Rust type* the enum is `#[non_exhaustive]`, even though it is complete
/// over the ISO 4217 active set: no user-defined currencies exist in `1.0`, but a
/// variant (a newly-issued ISO code, or a future `Custom`) can be added without a
/// breaking change (ADR-0034). Downstream `match`es therefore need a wildcard arm.
///
/// # Ordering
///
/// [`Ord`] is derived, and the variants are declared in **alphabetical order of
/// ISO 4217 code**, so `a < b` is exactly `a.code() < b.code()`. The crate commits
/// to keeping it that way: **a newly-issued ISO code is inserted in its
/// alphabetical position** (in the enum and in [`ALL`](Self::ALL) alike), never
/// appended (ADR-0053).
///
/// That single rule preserves both properties a caller could reasonably rely on.
/// The order stays alphabetical, and — because an insertion into a sorted sequence
/// does not transpose anything already in it — the relative order of any two
/// *existing* currencies is unchanged by the addition. A `BTreeMap<Currency, _>`,
/// a sorted report, or a persisted sort key gains the new code in its proper place
/// without reshuffling the rest.
///
/// What is **not** guaranteed is the numeric discriminant (`Currency::Usd as u16`),
/// which shifts whenever an alphabetically-earlier variant is added. It is an
/// implementation detail; persist [`code`](Self::code) instead. Nor is the ISO
/// [`numeric`](Self::numeric) code correlated with the ordering — ISO assigns those
/// non-alphabetically.
///
/// Should a non-ISO variant ever be added (ADR-0034 leaves the door open for
/// `Custom`), it has no ISO code to sort by and would be declared last; the
/// guarantee then reads "alphabetical among the ISO codes, non-ISO variants after
/// them", which still transposes no existing pair.
///
/// ```
/// use time_value::Currency;
///
/// assert!(Currency::Eur < Currency::Usd); // alphabetical by ISO code
/// assert!(Currency::ALL.windows(2).all(|w| w[0].code() < w[1].code()));
///
/// assert_eq!(Currency::Usd.code(), "USD");
/// assert_eq!(Currency::Usd.numeric(), 840);
/// assert_eq!(Currency::Usd.minor_unit_exponent(), Some(2));
/// assert_eq!(Currency::Jpy.minor_unit_exponent(), Some(0));
/// assert_eq!(Currency::Xxx.minor_unit_exponent(), None); // no minor unit
/// assert_eq!(Currency::from_code("EUR"), Some(Currency::Eur));
/// assert_eq!(Currency::from_code("ZZZ"), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Currency {
    /// UAE Dirham (AED, 784).
    Aed,
    /// Afghani (AFN, 971).
    Afn,
    /// Lek (ALL, 8).
    All,
    /// Armenian Dram (AMD, 51).
    Amd,
    /// Kwanza (AOA, 973).
    Aoa,
    /// Argentine Peso (ARS, 32).
    Ars,
    /// Australian Dollar (AUD, 36).
    Aud,
    /// Aruban Florin (AWG, 533).
    Awg,
    /// Azerbaijan Manat (AZN, 944).
    Azn,
    /// Convertible Mark (BAM, 977).
    Bam,
    /// Barbados Dollar (BBD, 52).
    Bbd,
    /// Taka (BDT, 50).
    Bdt,
    /// Bahraini Dinar (BHD, 48).
    Bhd,
    /// Burundi Franc (BIF, 108).
    Bif,
    /// Bermudian Dollar (BMD, 60).
    Bmd,
    /// Brunei Dollar (BND, 96).
    Bnd,
    /// Boliviano (BOB, 68).
    Bob,
    /// Mvdol (BOV, 984).
    Bov,
    /// Brazilian Real (BRL, 986).
    Brl,
    /// Bahamian Dollar (BSD, 44).
    Bsd,
    /// Ngultrum (BTN, 64).
    Btn,
    /// Pula (BWP, 72).
    Bwp,
    /// Belarusian Ruble (BYN, 933).
    Byn,
    /// Belize Dollar (BZD, 84).
    Bzd,
    /// Canadian Dollar (CAD, 124).
    Cad,
    /// Congolese Franc (CDF, 976).
    Cdf,
    /// WIR Euro (CHE, 947).
    Che,
    /// Swiss Franc (CHF, 756).
    Chf,
    /// WIR Franc (CHW, 948).
    Chw,
    /// Unidad de Fomento (CLF, 990).
    Clf,
    /// Chilean Peso (CLP, 152).
    Clp,
    /// Yuan Renminbi (CNY, 156).
    Cny,
    /// Colombian Peso (COP, 170).
    Cop,
    /// Unidad de Valor Real (COU, 970).
    Cou,
    /// Costa Rican Colon (CRC, 188).
    Crc,
    /// Cuban Peso (CUP, 192).
    Cup,
    /// Cabo Verde Escudo (CVE, 132).
    Cve,
    /// Czech Koruna (CZK, 203).
    Czk,
    /// Djibouti Franc (DJF, 262).
    Djf,
    /// Danish Krone (DKK, 208).
    Dkk,
    /// Dominican Peso (DOP, 214).
    Dop,
    /// Algerian Dinar (DZD, 12).
    Dzd,
    /// Egyptian Pound (EGP, 818).
    Egp,
    /// Nakfa (ERN, 232).
    Ern,
    /// Ethiopian Birr (ETB, 230).
    Etb,
    /// Euro (EUR, 978).
    Eur,
    /// Fiji Dollar (FJD, 242).
    Fjd,
    /// Falkland Islands Pound (FKP, 238).
    Fkp,
    /// Pound Sterling (GBP, 826).
    Gbp,
    /// Lari (GEL, 981).
    Gel,
    /// Ghana Cedi (GHS, 936).
    Ghs,
    /// Gibraltar Pound (GIP, 292).
    Gip,
    /// Dalasi (GMD, 270).
    Gmd,
    /// Guinean Franc (GNF, 324).
    Gnf,
    /// Quetzal (GTQ, 320).
    Gtq,
    /// Guyana Dollar (GYD, 328).
    Gyd,
    /// Hong Kong Dollar (HKD, 344).
    Hkd,
    /// Lempira (HNL, 340).
    Hnl,
    /// Gourde (HTG, 332).
    Htg,
    /// Forint (HUF, 348).
    Huf,
    /// Rupiah (IDR, 360).
    Idr,
    /// New Israeli Sheqel (ILS, 376).
    Ils,
    /// Indian Rupee (INR, 356).
    Inr,
    /// Iraqi Dinar (IQD, 368).
    Iqd,
    /// Iranian Rial (IRR, 364).
    Irr,
    /// Iceland Krona (ISK, 352).
    Isk,
    /// Jamaican Dollar (JMD, 388).
    Jmd,
    /// Jordanian Dinar (JOD, 400).
    Jod,
    /// Yen (JPY, 392).
    Jpy,
    /// Kenyan Shilling (KES, 404).
    Kes,
    /// Som (KGS, 417).
    Kgs,
    /// Riel (KHR, 116).
    Khr,
    /// Comorian Franc (KMF, 174).
    Kmf,
    /// North Korean Won (KPW, 408).
    Kpw,
    /// Won (KRW, 410).
    Krw,
    /// Kuwaiti Dinar (KWD, 414).
    Kwd,
    /// Cayman Islands Dollar (KYD, 136).
    Kyd,
    /// Tenge (KZT, 398).
    Kzt,
    /// Lao Kip (LAK, 418).
    Lak,
    /// Lebanese Pound (LBP, 422).
    Lbp,
    /// Sri Lanka Rupee (LKR, 144).
    Lkr,
    /// Liberian Dollar (LRD, 430).
    Lrd,
    /// Loti (LSL, 426).
    Lsl,
    /// Libyan Dinar (LYD, 434).
    Lyd,
    /// Moroccan Dirham (MAD, 504).
    Mad,
    /// Moldovan Leu (MDL, 498).
    Mdl,
    /// Malagasy Ariary (MGA, 969).
    Mga,
    /// Denar (MKD, 807).
    Mkd,
    /// Kyat (MMK, 104).
    Mmk,
    /// Tugrik (MNT, 496).
    Mnt,
    /// Pataca (MOP, 446).
    Mop,
    /// Ouguiya (MRU, 929).
    Mru,
    /// Mauritius Rupee (MUR, 480).
    Mur,
    /// Rufiyaa (MVR, 462).
    Mvr,
    /// Malawi Kwacha (MWK, 454).
    Mwk,
    /// Mexican Peso (MXN, 484).
    Mxn,
    /// Mexican Unidad de Inversion (UDI) (MXV, 979).
    Mxv,
    /// Malaysian Ringgit (MYR, 458).
    Myr,
    /// Mozambique Metical (MZN, 943).
    Mzn,
    /// Namibia Dollar (NAD, 516).
    Nad,
    /// Naira (NGN, 566).
    Ngn,
    /// Cordoba Oro (NIO, 558).
    Nio,
    /// Norwegian Krone (NOK, 578).
    Nok,
    /// Nepalese Rupee (NPR, 524).
    Npr,
    /// New Zealand Dollar (NZD, 554).
    Nzd,
    /// Rial Omani (OMR, 512).
    Omr,
    /// Balboa (PAB, 590).
    Pab,
    /// Sol (PEN, 604).
    Pen,
    /// Kina (PGK, 598).
    Pgk,
    /// Philippine Peso (PHP, 608).
    Php,
    /// Pakistan Rupee (PKR, 586).
    Pkr,
    /// Zloty (PLN, 985).
    Pln,
    /// Guarani (PYG, 600).
    Pyg,
    /// Qatari Rial (QAR, 634).
    Qar,
    /// Romanian Leu (RON, 946).
    Ron,
    /// Serbian Dinar (RSD, 941).
    Rsd,
    /// Russian Ruble (RUB, 643).
    Rub,
    /// Rwanda Franc (RWF, 646).
    Rwf,
    /// Saudi Riyal (SAR, 682).
    Sar,
    /// Solomon Islands Dollar (SBD, 90).
    Sbd,
    /// Seychelles Rupee (SCR, 690).
    Scr,
    /// Sudanese Pound (SDG, 938).
    Sdg,
    /// Swedish Krona (SEK, 752).
    Sek,
    /// Singapore Dollar (SGD, 702).
    Sgd,
    /// Saint Helena Pound (SHP, 654).
    Shp,
    /// Leone (SLE, 925).
    Sle,
    /// Somali Shilling (SOS, 706).
    Sos,
    /// Surinam Dollar (SRD, 968).
    Srd,
    /// South Sudanese Pound (SSP, 728).
    Ssp,
    /// Dobra (STN, 930).
    Stn,
    /// El Salvador Colon (SVC, 222).
    Svc,
    /// Syrian Pound (SYP, 760).
    Syp,
    /// Lilangeni (SZL, 748).
    Szl,
    /// Baht (THB, 764).
    Thb,
    /// Somoni (TJS, 972).
    Tjs,
    /// Turkmenistan New Manat (TMT, 934).
    Tmt,
    /// Tunisian Dinar (TND, 788).
    Tnd,
    /// Pa'anga (TOP, 776).
    Top,
    /// Turkish Lira (TRY, 949).
    Try,
    /// Trinidad and Tobago Dollar (TTD, 780).
    Ttd,
    /// New Taiwan Dollar (TWD, 901).
    Twd,
    /// Tanzanian Shilling (TZS, 834).
    Tzs,
    /// Hryvnia (UAH, 980).
    Uah,
    /// Uganda Shilling (UGX, 800).
    Ugx,
    /// US Dollar (USD, 840).
    Usd,
    /// Uruguay Peso en Unidades Indexadas (UI) (UYI, 940).
    Uyi,
    /// Peso Uruguayo (UYU, 858).
    Uyu,
    /// Unidad Previsional (UYW, 927).
    Uyw,
    /// Uzbekistan Sum (UZS, 860).
    Uzs,
    /// Bolivar Soberano (Digital) (VED, 926).
    Ved,
    /// Bolivar Soberano (VES, 928).
    Ves,
    /// Dong (VND, 704).
    Vnd,
    /// Vatu (VUV, 548).
    Vuv,
    /// Tala (WST, 882).
    Wst,
    /// Arab Accounting Dinar (XAD, 396).
    Xad,
    /// CFA Franc BEAC (XAF, 950).
    Xaf,
    /// Silver (XAG, 961).
    Xag,
    /// Gold (XAU, 959).
    Xau,
    /// Bond Markets Unit European Composite Unit (EURCO) (XBA, 955).
    Xba,
    /// Bond Markets Unit European Monetary Unit (E.M.U.-6) (XBB, 956).
    Xbb,
    /// Bond Markets Unit European Unit of Account 9 (E.U.A.-9) (XBC, 957).
    Xbc,
    /// Bond Markets Unit European Unit of Account 17 (E.U.A.-17) (XBD, 958).
    Xbd,
    /// East Caribbean Dollar (XCD, 951).
    Xcd,
    /// Caribbean Guilder (XCG, 532).
    Xcg,
    /// SDR (Special Drawing Right) (XDR, 960).
    Xdr,
    /// CFA Franc BCEAO (XOF, 952).
    Xof,
    /// Palladium (XPD, 964).
    Xpd,
    /// CFP Franc (XPF, 953).
    Xpf,
    /// Platinum (XPT, 962).
    Xpt,
    /// Sucre (XSU, 994).
    Xsu,
    /// Code reserved for testing purposes (XTS, 963).
    Xts,
    /// ADB Unit of Account (XUA, 965).
    Xua,
    /// No currency (XXX, 999).
    Xxx,
    /// Yemeni Rial (YER, 886).
    Yer,
    /// Rand (ZAR, 710).
    Zar,
    /// Zambian Kwacha (ZMW, 967).
    Zmw,
    /// Zimbabwe Gold (ZWG, 924).
    Zwg,
}

/// Static metadata for one currency (ISO 4217 alphabetic code, numeric code,
/// and minor-unit exponent).
#[derive(Clone, Copy)]
struct Meta {
    code: &'static str,
    numeric: u16,
    minor: Option<u8>,
}

impl Currency {
    /// The ISO 4217 three-letter alphabetic code, e.g. `"USD"`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        self.meta().code
    }

    /// The ISO 4217 numeric code, e.g. `840` for `USD`.
    #[must_use]
    pub const fn numeric(self) -> u16 {
        self.meta().numeric
    }

    /// The minor-unit exponent — the number of decimal places in the currency's
    /// minor unit (`2` for `USD` cents, `0` for `JPY`, `3` for `BHD`).
    ///
    /// `None` for codes with no minor unit: [`Xxx`](Self::Xxx), the precious
    /// metals ([`Xau`](Self::Xau) &c.), and the fund/testing/unit-of-account
    /// codes. Used only for presentation rounding
    /// ([`Money::round_to_currency`][crate::Money::round_to_currency]); it never
    /// affects computation.
    #[cfg_attr(
        not(any(feature = "std", feature = "libm")),
        doc = "

[crate::Money::round_to_currency]: https://docs.rs/time_value/latest/time_value/struct.Money.html#method.round_to_currency
"
    )]
    #[must_use]
    pub const fn minor_unit_exponent(self) -> Option<u8> {
        self.meta().minor
    }

    /// Parses an ISO 4217 alphabetic code, e.g. `"USD"`.
    ///
    /// Total and canonical: the match is case-sensitive (uppercase, as ISO
    /// defines the codes) and returns `None` for any string that is not exactly
    /// one of the known codes. This is the strict lookup, and the one the
    /// `serde` wire format uses; the [`FromStr`] impl is the human-input door onto
    /// the same table and accepts any casing (ADR-0061).
    ///
    /// [`FromStr`]: core::str::FromStr
    #[must_use]
    #[allow(clippy::too_many_lines)] // one arm per ISO 4217 code (generated)
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "AED" => Self::Aed,
            "AFN" => Self::Afn,
            "ALL" => Self::All,
            "AMD" => Self::Amd,
            "AOA" => Self::Aoa,
            "ARS" => Self::Ars,
            "AUD" => Self::Aud,
            "AWG" => Self::Awg,
            "AZN" => Self::Azn,
            "BAM" => Self::Bam,
            "BBD" => Self::Bbd,
            "BDT" => Self::Bdt,
            "BHD" => Self::Bhd,
            "BIF" => Self::Bif,
            "BMD" => Self::Bmd,
            "BND" => Self::Bnd,
            "BOB" => Self::Bob,
            "BOV" => Self::Bov,
            "BRL" => Self::Brl,
            "BSD" => Self::Bsd,
            "BTN" => Self::Btn,
            "BWP" => Self::Bwp,
            "BYN" => Self::Byn,
            "BZD" => Self::Bzd,
            "CAD" => Self::Cad,
            "CDF" => Self::Cdf,
            "CHE" => Self::Che,
            "CHF" => Self::Chf,
            "CHW" => Self::Chw,
            "CLF" => Self::Clf,
            "CLP" => Self::Clp,
            "CNY" => Self::Cny,
            "COP" => Self::Cop,
            "COU" => Self::Cou,
            "CRC" => Self::Crc,
            "CUP" => Self::Cup,
            "CVE" => Self::Cve,
            "CZK" => Self::Czk,
            "DJF" => Self::Djf,
            "DKK" => Self::Dkk,
            "DOP" => Self::Dop,
            "DZD" => Self::Dzd,
            "EGP" => Self::Egp,
            "ERN" => Self::Ern,
            "ETB" => Self::Etb,
            "EUR" => Self::Eur,
            "FJD" => Self::Fjd,
            "FKP" => Self::Fkp,
            "GBP" => Self::Gbp,
            "GEL" => Self::Gel,
            "GHS" => Self::Ghs,
            "GIP" => Self::Gip,
            "GMD" => Self::Gmd,
            "GNF" => Self::Gnf,
            "GTQ" => Self::Gtq,
            "GYD" => Self::Gyd,
            "HKD" => Self::Hkd,
            "HNL" => Self::Hnl,
            "HTG" => Self::Htg,
            "HUF" => Self::Huf,
            "IDR" => Self::Idr,
            "ILS" => Self::Ils,
            "INR" => Self::Inr,
            "IQD" => Self::Iqd,
            "IRR" => Self::Irr,
            "ISK" => Self::Isk,
            "JMD" => Self::Jmd,
            "JOD" => Self::Jod,
            "JPY" => Self::Jpy,
            "KES" => Self::Kes,
            "KGS" => Self::Kgs,
            "KHR" => Self::Khr,
            "KMF" => Self::Kmf,
            "KPW" => Self::Kpw,
            "KRW" => Self::Krw,
            "KWD" => Self::Kwd,
            "KYD" => Self::Kyd,
            "KZT" => Self::Kzt,
            "LAK" => Self::Lak,
            "LBP" => Self::Lbp,
            "LKR" => Self::Lkr,
            "LRD" => Self::Lrd,
            "LSL" => Self::Lsl,
            "LYD" => Self::Lyd,
            "MAD" => Self::Mad,
            "MDL" => Self::Mdl,
            "MGA" => Self::Mga,
            "MKD" => Self::Mkd,
            "MMK" => Self::Mmk,
            "MNT" => Self::Mnt,
            "MOP" => Self::Mop,
            "MRU" => Self::Mru,
            "MUR" => Self::Mur,
            "MVR" => Self::Mvr,
            "MWK" => Self::Mwk,
            "MXN" => Self::Mxn,
            "MXV" => Self::Mxv,
            "MYR" => Self::Myr,
            "MZN" => Self::Mzn,
            "NAD" => Self::Nad,
            "NGN" => Self::Ngn,
            "NIO" => Self::Nio,
            "NOK" => Self::Nok,
            "NPR" => Self::Npr,
            "NZD" => Self::Nzd,
            "OMR" => Self::Omr,
            "PAB" => Self::Pab,
            "PEN" => Self::Pen,
            "PGK" => Self::Pgk,
            "PHP" => Self::Php,
            "PKR" => Self::Pkr,
            "PLN" => Self::Pln,
            "PYG" => Self::Pyg,
            "QAR" => Self::Qar,
            "RON" => Self::Ron,
            "RSD" => Self::Rsd,
            "RUB" => Self::Rub,
            "RWF" => Self::Rwf,
            "SAR" => Self::Sar,
            "SBD" => Self::Sbd,
            "SCR" => Self::Scr,
            "SDG" => Self::Sdg,
            "SEK" => Self::Sek,
            "SGD" => Self::Sgd,
            "SHP" => Self::Shp,
            "SLE" => Self::Sle,
            "SOS" => Self::Sos,
            "SRD" => Self::Srd,
            "SSP" => Self::Ssp,
            "STN" => Self::Stn,
            "SVC" => Self::Svc,
            "SYP" => Self::Syp,
            "SZL" => Self::Szl,
            "THB" => Self::Thb,
            "TJS" => Self::Tjs,
            "TMT" => Self::Tmt,
            "TND" => Self::Tnd,
            "TOP" => Self::Top,
            "TRY" => Self::Try,
            "TTD" => Self::Ttd,
            "TWD" => Self::Twd,
            "TZS" => Self::Tzs,
            "UAH" => Self::Uah,
            "UGX" => Self::Ugx,
            "USD" => Self::Usd,
            "UYI" => Self::Uyi,
            "UYU" => Self::Uyu,
            "UYW" => Self::Uyw,
            "UZS" => Self::Uzs,
            "VED" => Self::Ved,
            "VES" => Self::Ves,
            "VND" => Self::Vnd,
            "VUV" => Self::Vuv,
            "WST" => Self::Wst,
            "XAD" => Self::Xad,
            "XAF" => Self::Xaf,
            "XAG" => Self::Xag,
            "XAU" => Self::Xau,
            "XBA" => Self::Xba,
            "XBB" => Self::Xbb,
            "XBC" => Self::Xbc,
            "XBD" => Self::Xbd,
            "XCD" => Self::Xcd,
            "XCG" => Self::Xcg,
            "XDR" => Self::Xdr,
            "XOF" => Self::Xof,
            "XPD" => Self::Xpd,
            "XPF" => Self::Xpf,
            "XPT" => Self::Xpt,
            "XSU" => Self::Xsu,
            "XTS" => Self::Xts,
            "XUA" => Self::Xua,
            "XXX" => Self::Xxx,
            "YER" => Self::Yer,
            "ZAR" => Self::Zar,
            "ZMW" => Self::Zmw,
            "ZWG" => Self::Zwg,
            _ => return None,
        })
    }

    /// Every currency in the enum, in variant order — the full ISO 4217
    /// active set plus the reserved codes. Lets callers enumerate the closed
    /// set (e.g. to advertise it in a schema); the binaries use it for the
    /// currency-code input `enum` (ADR-0039).
    pub const ALL: &[Currency] = &[
        Currency::Aed,
        Currency::Afn,
        Currency::All,
        Currency::Amd,
        Currency::Aoa,
        Currency::Ars,
        Currency::Aud,
        Currency::Awg,
        Currency::Azn,
        Currency::Bam,
        Currency::Bbd,
        Currency::Bdt,
        Currency::Bhd,
        Currency::Bif,
        Currency::Bmd,
        Currency::Bnd,
        Currency::Bob,
        Currency::Bov,
        Currency::Brl,
        Currency::Bsd,
        Currency::Btn,
        Currency::Bwp,
        Currency::Byn,
        Currency::Bzd,
        Currency::Cad,
        Currency::Cdf,
        Currency::Che,
        Currency::Chf,
        Currency::Chw,
        Currency::Clf,
        Currency::Clp,
        Currency::Cny,
        Currency::Cop,
        Currency::Cou,
        Currency::Crc,
        Currency::Cup,
        Currency::Cve,
        Currency::Czk,
        Currency::Djf,
        Currency::Dkk,
        Currency::Dop,
        Currency::Dzd,
        Currency::Egp,
        Currency::Ern,
        Currency::Etb,
        Currency::Eur,
        Currency::Fjd,
        Currency::Fkp,
        Currency::Gbp,
        Currency::Gel,
        Currency::Ghs,
        Currency::Gip,
        Currency::Gmd,
        Currency::Gnf,
        Currency::Gtq,
        Currency::Gyd,
        Currency::Hkd,
        Currency::Hnl,
        Currency::Htg,
        Currency::Huf,
        Currency::Idr,
        Currency::Ils,
        Currency::Inr,
        Currency::Iqd,
        Currency::Irr,
        Currency::Isk,
        Currency::Jmd,
        Currency::Jod,
        Currency::Jpy,
        Currency::Kes,
        Currency::Kgs,
        Currency::Khr,
        Currency::Kmf,
        Currency::Kpw,
        Currency::Krw,
        Currency::Kwd,
        Currency::Kyd,
        Currency::Kzt,
        Currency::Lak,
        Currency::Lbp,
        Currency::Lkr,
        Currency::Lrd,
        Currency::Lsl,
        Currency::Lyd,
        Currency::Mad,
        Currency::Mdl,
        Currency::Mga,
        Currency::Mkd,
        Currency::Mmk,
        Currency::Mnt,
        Currency::Mop,
        Currency::Mru,
        Currency::Mur,
        Currency::Mvr,
        Currency::Mwk,
        Currency::Mxn,
        Currency::Mxv,
        Currency::Myr,
        Currency::Mzn,
        Currency::Nad,
        Currency::Ngn,
        Currency::Nio,
        Currency::Nok,
        Currency::Npr,
        Currency::Nzd,
        Currency::Omr,
        Currency::Pab,
        Currency::Pen,
        Currency::Pgk,
        Currency::Php,
        Currency::Pkr,
        Currency::Pln,
        Currency::Pyg,
        Currency::Qar,
        Currency::Ron,
        Currency::Rsd,
        Currency::Rub,
        Currency::Rwf,
        Currency::Sar,
        Currency::Sbd,
        Currency::Scr,
        Currency::Sdg,
        Currency::Sek,
        Currency::Sgd,
        Currency::Shp,
        Currency::Sle,
        Currency::Sos,
        Currency::Srd,
        Currency::Ssp,
        Currency::Stn,
        Currency::Svc,
        Currency::Syp,
        Currency::Szl,
        Currency::Thb,
        Currency::Tjs,
        Currency::Tmt,
        Currency::Tnd,
        Currency::Top,
        Currency::Try,
        Currency::Ttd,
        Currency::Twd,
        Currency::Tzs,
        Currency::Uah,
        Currency::Ugx,
        Currency::Usd,
        Currency::Uyi,
        Currency::Uyu,
        Currency::Uyw,
        Currency::Uzs,
        Currency::Ved,
        Currency::Ves,
        Currency::Vnd,
        Currency::Vuv,
        Currency::Wst,
        Currency::Xad,
        Currency::Xaf,
        Currency::Xag,
        Currency::Xau,
        Currency::Xba,
        Currency::Xbb,
        Currency::Xbc,
        Currency::Xbd,
        Currency::Xcd,
        Currency::Xcg,
        Currency::Xdr,
        Currency::Xof,
        Currency::Xpd,
        Currency::Xpf,
        Currency::Xpt,
        Currency::Xsu,
        Currency::Xts,
        Currency::Xua,
        Currency::Xxx,
        Currency::Yer,
        Currency::Zar,
        Currency::Zmw,
        Currency::Zwg,
    ];

    #[allow(clippy::too_many_lines)] // one arm per ISO 4217 code (generated)
    const fn meta(self) -> Meta {
        match self {
            Self::Aed => Meta {
                code: "AED",
                numeric: 784,
                minor: Some(2),
            },
            Self::Afn => Meta {
                code: "AFN",
                numeric: 971,
                minor: Some(2),
            },
            Self::All => Meta {
                code: "ALL",
                numeric: 8,
                minor: Some(2),
            },
            Self::Amd => Meta {
                code: "AMD",
                numeric: 51,
                minor: Some(2),
            },
            Self::Aoa => Meta {
                code: "AOA",
                numeric: 973,
                minor: Some(2),
            },
            Self::Ars => Meta {
                code: "ARS",
                numeric: 32,
                minor: Some(2),
            },
            Self::Aud => Meta {
                code: "AUD",
                numeric: 36,
                minor: Some(2),
            },
            Self::Awg => Meta {
                code: "AWG",
                numeric: 533,
                minor: Some(2),
            },
            Self::Azn => Meta {
                code: "AZN",
                numeric: 944,
                minor: Some(2),
            },
            Self::Bam => Meta {
                code: "BAM",
                numeric: 977,
                minor: Some(2),
            },
            Self::Bbd => Meta {
                code: "BBD",
                numeric: 52,
                minor: Some(2),
            },
            Self::Bdt => Meta {
                code: "BDT",
                numeric: 50,
                minor: Some(2),
            },
            Self::Bhd => Meta {
                code: "BHD",
                numeric: 48,
                minor: Some(3),
            },
            Self::Bif => Meta {
                code: "BIF",
                numeric: 108,
                minor: Some(0),
            },
            Self::Bmd => Meta {
                code: "BMD",
                numeric: 60,
                minor: Some(2),
            },
            Self::Bnd => Meta {
                code: "BND",
                numeric: 96,
                minor: Some(2),
            },
            Self::Bob => Meta {
                code: "BOB",
                numeric: 68,
                minor: Some(2),
            },
            Self::Bov => Meta {
                code: "BOV",
                numeric: 984,
                minor: Some(2),
            },
            Self::Brl => Meta {
                code: "BRL",
                numeric: 986,
                minor: Some(2),
            },
            Self::Bsd => Meta {
                code: "BSD",
                numeric: 44,
                minor: Some(2),
            },
            Self::Btn => Meta {
                code: "BTN",
                numeric: 64,
                minor: Some(2),
            },
            Self::Bwp => Meta {
                code: "BWP",
                numeric: 72,
                minor: Some(2),
            },
            Self::Byn => Meta {
                code: "BYN",
                numeric: 933,
                minor: Some(2),
            },
            Self::Bzd => Meta {
                code: "BZD",
                numeric: 84,
                minor: Some(2),
            },
            Self::Cad => Meta {
                code: "CAD",
                numeric: 124,
                minor: Some(2),
            },
            Self::Cdf => Meta {
                code: "CDF",
                numeric: 976,
                minor: Some(2),
            },
            Self::Che => Meta {
                code: "CHE",
                numeric: 947,
                minor: Some(2),
            },
            Self::Chf => Meta {
                code: "CHF",
                numeric: 756,
                minor: Some(2),
            },
            Self::Chw => Meta {
                code: "CHW",
                numeric: 948,
                minor: Some(2),
            },
            Self::Clf => Meta {
                code: "CLF",
                numeric: 990,
                minor: Some(4),
            },
            Self::Clp => Meta {
                code: "CLP",
                numeric: 152,
                minor: Some(0),
            },
            Self::Cny => Meta {
                code: "CNY",
                numeric: 156,
                minor: Some(2),
            },
            Self::Cop => Meta {
                code: "COP",
                numeric: 170,
                minor: Some(2),
            },
            Self::Cou => Meta {
                code: "COU",
                numeric: 970,
                minor: Some(2),
            },
            Self::Crc => Meta {
                code: "CRC",
                numeric: 188,
                minor: Some(2),
            },
            Self::Cup => Meta {
                code: "CUP",
                numeric: 192,
                minor: Some(2),
            },
            Self::Cve => Meta {
                code: "CVE",
                numeric: 132,
                minor: Some(2),
            },
            Self::Czk => Meta {
                code: "CZK",
                numeric: 203,
                minor: Some(2),
            },
            Self::Djf => Meta {
                code: "DJF",
                numeric: 262,
                minor: Some(0),
            },
            Self::Dkk => Meta {
                code: "DKK",
                numeric: 208,
                minor: Some(2),
            },
            Self::Dop => Meta {
                code: "DOP",
                numeric: 214,
                minor: Some(2),
            },
            Self::Dzd => Meta {
                code: "DZD",
                numeric: 12,
                minor: Some(2),
            },
            Self::Egp => Meta {
                code: "EGP",
                numeric: 818,
                minor: Some(2),
            },
            Self::Ern => Meta {
                code: "ERN",
                numeric: 232,
                minor: Some(2),
            },
            Self::Etb => Meta {
                code: "ETB",
                numeric: 230,
                minor: Some(2),
            },
            Self::Eur => Meta {
                code: "EUR",
                numeric: 978,
                minor: Some(2),
            },
            Self::Fjd => Meta {
                code: "FJD",
                numeric: 242,
                minor: Some(2),
            },
            Self::Fkp => Meta {
                code: "FKP",
                numeric: 238,
                minor: Some(2),
            },
            Self::Gbp => Meta {
                code: "GBP",
                numeric: 826,
                minor: Some(2),
            },
            Self::Gel => Meta {
                code: "GEL",
                numeric: 981,
                minor: Some(2),
            },
            Self::Ghs => Meta {
                code: "GHS",
                numeric: 936,
                minor: Some(2),
            },
            Self::Gip => Meta {
                code: "GIP",
                numeric: 292,
                minor: Some(2),
            },
            Self::Gmd => Meta {
                code: "GMD",
                numeric: 270,
                minor: Some(2),
            },
            Self::Gnf => Meta {
                code: "GNF",
                numeric: 324,
                minor: Some(0),
            },
            Self::Gtq => Meta {
                code: "GTQ",
                numeric: 320,
                minor: Some(2),
            },
            Self::Gyd => Meta {
                code: "GYD",
                numeric: 328,
                minor: Some(2),
            },
            Self::Hkd => Meta {
                code: "HKD",
                numeric: 344,
                minor: Some(2),
            },
            Self::Hnl => Meta {
                code: "HNL",
                numeric: 340,
                minor: Some(2),
            },
            Self::Htg => Meta {
                code: "HTG",
                numeric: 332,
                minor: Some(2),
            },
            Self::Huf => Meta {
                code: "HUF",
                numeric: 348,
                minor: Some(2),
            },
            Self::Idr => Meta {
                code: "IDR",
                numeric: 360,
                minor: Some(2),
            },
            Self::Ils => Meta {
                code: "ILS",
                numeric: 376,
                minor: Some(2),
            },
            Self::Inr => Meta {
                code: "INR",
                numeric: 356,
                minor: Some(2),
            },
            Self::Iqd => Meta {
                code: "IQD",
                numeric: 368,
                minor: Some(3),
            },
            Self::Irr => Meta {
                code: "IRR",
                numeric: 364,
                minor: Some(2),
            },
            Self::Isk => Meta {
                code: "ISK",
                numeric: 352,
                minor: Some(0),
            },
            Self::Jmd => Meta {
                code: "JMD",
                numeric: 388,
                minor: Some(2),
            },
            Self::Jod => Meta {
                code: "JOD",
                numeric: 400,
                minor: Some(3),
            },
            Self::Jpy => Meta {
                code: "JPY",
                numeric: 392,
                minor: Some(0),
            },
            Self::Kes => Meta {
                code: "KES",
                numeric: 404,
                minor: Some(2),
            },
            Self::Kgs => Meta {
                code: "KGS",
                numeric: 417,
                minor: Some(2),
            },
            Self::Khr => Meta {
                code: "KHR",
                numeric: 116,
                minor: Some(2),
            },
            Self::Kmf => Meta {
                code: "KMF",
                numeric: 174,
                minor: Some(0),
            },
            Self::Kpw => Meta {
                code: "KPW",
                numeric: 408,
                minor: Some(2),
            },
            Self::Krw => Meta {
                code: "KRW",
                numeric: 410,
                minor: Some(0),
            },
            Self::Kwd => Meta {
                code: "KWD",
                numeric: 414,
                minor: Some(3),
            },
            Self::Kyd => Meta {
                code: "KYD",
                numeric: 136,
                minor: Some(2),
            },
            Self::Kzt => Meta {
                code: "KZT",
                numeric: 398,
                minor: Some(2),
            },
            Self::Lak => Meta {
                code: "LAK",
                numeric: 418,
                minor: Some(2),
            },
            Self::Lbp => Meta {
                code: "LBP",
                numeric: 422,
                minor: Some(2),
            },
            Self::Lkr => Meta {
                code: "LKR",
                numeric: 144,
                minor: Some(2),
            },
            Self::Lrd => Meta {
                code: "LRD",
                numeric: 430,
                minor: Some(2),
            },
            Self::Lsl => Meta {
                code: "LSL",
                numeric: 426,
                minor: Some(2),
            },
            Self::Lyd => Meta {
                code: "LYD",
                numeric: 434,
                minor: Some(3),
            },
            Self::Mad => Meta {
                code: "MAD",
                numeric: 504,
                minor: Some(2),
            },
            Self::Mdl => Meta {
                code: "MDL",
                numeric: 498,
                minor: Some(2),
            },
            Self::Mga => Meta {
                code: "MGA",
                numeric: 969,
                minor: Some(2),
            },
            Self::Mkd => Meta {
                code: "MKD",
                numeric: 807,
                minor: Some(2),
            },
            Self::Mmk => Meta {
                code: "MMK",
                numeric: 104,
                minor: Some(2),
            },
            Self::Mnt => Meta {
                code: "MNT",
                numeric: 496,
                minor: Some(2),
            },
            Self::Mop => Meta {
                code: "MOP",
                numeric: 446,
                minor: Some(2),
            },
            Self::Mru => Meta {
                code: "MRU",
                numeric: 929,
                minor: Some(2),
            },
            Self::Mur => Meta {
                code: "MUR",
                numeric: 480,
                minor: Some(2),
            },
            Self::Mvr => Meta {
                code: "MVR",
                numeric: 462,
                minor: Some(2),
            },
            Self::Mwk => Meta {
                code: "MWK",
                numeric: 454,
                minor: Some(2),
            },
            Self::Mxn => Meta {
                code: "MXN",
                numeric: 484,
                minor: Some(2),
            },
            Self::Mxv => Meta {
                code: "MXV",
                numeric: 979,
                minor: Some(2),
            },
            Self::Myr => Meta {
                code: "MYR",
                numeric: 458,
                minor: Some(2),
            },
            Self::Mzn => Meta {
                code: "MZN",
                numeric: 943,
                minor: Some(2),
            },
            Self::Nad => Meta {
                code: "NAD",
                numeric: 516,
                minor: Some(2),
            },
            Self::Ngn => Meta {
                code: "NGN",
                numeric: 566,
                minor: Some(2),
            },
            Self::Nio => Meta {
                code: "NIO",
                numeric: 558,
                minor: Some(2),
            },
            Self::Nok => Meta {
                code: "NOK",
                numeric: 578,
                minor: Some(2),
            },
            Self::Npr => Meta {
                code: "NPR",
                numeric: 524,
                minor: Some(2),
            },
            Self::Nzd => Meta {
                code: "NZD",
                numeric: 554,
                minor: Some(2),
            },
            Self::Omr => Meta {
                code: "OMR",
                numeric: 512,
                minor: Some(3),
            },
            Self::Pab => Meta {
                code: "PAB",
                numeric: 590,
                minor: Some(2),
            },
            Self::Pen => Meta {
                code: "PEN",
                numeric: 604,
                minor: Some(2),
            },
            Self::Pgk => Meta {
                code: "PGK",
                numeric: 598,
                minor: Some(2),
            },
            Self::Php => Meta {
                code: "PHP",
                numeric: 608,
                minor: Some(2),
            },
            Self::Pkr => Meta {
                code: "PKR",
                numeric: 586,
                minor: Some(2),
            },
            Self::Pln => Meta {
                code: "PLN",
                numeric: 985,
                minor: Some(2),
            },
            Self::Pyg => Meta {
                code: "PYG",
                numeric: 600,
                minor: Some(0),
            },
            Self::Qar => Meta {
                code: "QAR",
                numeric: 634,
                minor: Some(2),
            },
            Self::Ron => Meta {
                code: "RON",
                numeric: 946,
                minor: Some(2),
            },
            Self::Rsd => Meta {
                code: "RSD",
                numeric: 941,
                minor: Some(2),
            },
            Self::Rub => Meta {
                code: "RUB",
                numeric: 643,
                minor: Some(2),
            },
            Self::Rwf => Meta {
                code: "RWF",
                numeric: 646,
                minor: Some(0),
            },
            Self::Sar => Meta {
                code: "SAR",
                numeric: 682,
                minor: Some(2),
            },
            Self::Sbd => Meta {
                code: "SBD",
                numeric: 90,
                minor: Some(2),
            },
            Self::Scr => Meta {
                code: "SCR",
                numeric: 690,
                minor: Some(2),
            },
            Self::Sdg => Meta {
                code: "SDG",
                numeric: 938,
                minor: Some(2),
            },
            Self::Sek => Meta {
                code: "SEK",
                numeric: 752,
                minor: Some(2),
            },
            Self::Sgd => Meta {
                code: "SGD",
                numeric: 702,
                minor: Some(2),
            },
            Self::Shp => Meta {
                code: "SHP",
                numeric: 654,
                minor: Some(2),
            },
            Self::Sle => Meta {
                code: "SLE",
                numeric: 925,
                minor: Some(2),
            },
            Self::Sos => Meta {
                code: "SOS",
                numeric: 706,
                minor: Some(2),
            },
            Self::Srd => Meta {
                code: "SRD",
                numeric: 968,
                minor: Some(2),
            },
            Self::Ssp => Meta {
                code: "SSP",
                numeric: 728,
                minor: Some(2),
            },
            Self::Stn => Meta {
                code: "STN",
                numeric: 930,
                minor: Some(2),
            },
            Self::Svc => Meta {
                code: "SVC",
                numeric: 222,
                minor: Some(2),
            },
            Self::Syp => Meta {
                code: "SYP",
                numeric: 760,
                minor: Some(2),
            },
            Self::Szl => Meta {
                code: "SZL",
                numeric: 748,
                minor: Some(2),
            },
            Self::Thb => Meta {
                code: "THB",
                numeric: 764,
                minor: Some(2),
            },
            Self::Tjs => Meta {
                code: "TJS",
                numeric: 972,
                minor: Some(2),
            },
            Self::Tmt => Meta {
                code: "TMT",
                numeric: 934,
                minor: Some(2),
            },
            Self::Tnd => Meta {
                code: "TND",
                numeric: 788,
                minor: Some(3),
            },
            Self::Top => Meta {
                code: "TOP",
                numeric: 776,
                minor: Some(2),
            },
            Self::Try => Meta {
                code: "TRY",
                numeric: 949,
                minor: Some(2),
            },
            Self::Ttd => Meta {
                code: "TTD",
                numeric: 780,
                minor: Some(2),
            },
            Self::Twd => Meta {
                code: "TWD",
                numeric: 901,
                minor: Some(2),
            },
            Self::Tzs => Meta {
                code: "TZS",
                numeric: 834,
                minor: Some(2),
            },
            Self::Uah => Meta {
                code: "UAH",
                numeric: 980,
                minor: Some(2),
            },
            Self::Ugx => Meta {
                code: "UGX",
                numeric: 800,
                minor: Some(0),
            },
            Self::Usd => Meta {
                code: "USD",
                numeric: 840,
                minor: Some(2),
            },
            Self::Uyi => Meta {
                code: "UYI",
                numeric: 940,
                minor: Some(0),
            },
            Self::Uyu => Meta {
                code: "UYU",
                numeric: 858,
                minor: Some(2),
            },
            Self::Uyw => Meta {
                code: "UYW",
                numeric: 927,
                minor: Some(4),
            },
            Self::Uzs => Meta {
                code: "UZS",
                numeric: 860,
                minor: Some(2),
            },
            Self::Ved => Meta {
                code: "VED",
                numeric: 926,
                minor: Some(2),
            },
            Self::Ves => Meta {
                code: "VES",
                numeric: 928,
                minor: Some(2),
            },
            Self::Vnd => Meta {
                code: "VND",
                numeric: 704,
                minor: Some(0),
            },
            Self::Vuv => Meta {
                code: "VUV",
                numeric: 548,
                minor: Some(0),
            },
            Self::Wst => Meta {
                code: "WST",
                numeric: 882,
                minor: Some(2),
            },
            Self::Xad => Meta {
                code: "XAD",
                numeric: 396,
                minor: Some(2),
            },
            Self::Xaf => Meta {
                code: "XAF",
                numeric: 950,
                minor: Some(0),
            },
            Self::Xag => Meta {
                code: "XAG",
                numeric: 961,
                minor: None,
            },
            Self::Xau => Meta {
                code: "XAU",
                numeric: 959,
                minor: None,
            },
            Self::Xba => Meta {
                code: "XBA",
                numeric: 955,
                minor: None,
            },
            Self::Xbb => Meta {
                code: "XBB",
                numeric: 956,
                minor: None,
            },
            Self::Xbc => Meta {
                code: "XBC",
                numeric: 957,
                minor: None,
            },
            Self::Xbd => Meta {
                code: "XBD",
                numeric: 958,
                minor: None,
            },
            Self::Xcd => Meta {
                code: "XCD",
                numeric: 951,
                minor: Some(2),
            },
            Self::Xcg => Meta {
                code: "XCG",
                numeric: 532,
                minor: Some(2),
            },
            Self::Xdr => Meta {
                code: "XDR",
                numeric: 960,
                minor: None,
            },
            Self::Xof => Meta {
                code: "XOF",
                numeric: 952,
                minor: Some(0),
            },
            Self::Xpd => Meta {
                code: "XPD",
                numeric: 964,
                minor: None,
            },
            Self::Xpf => Meta {
                code: "XPF",
                numeric: 953,
                minor: Some(0),
            },
            Self::Xpt => Meta {
                code: "XPT",
                numeric: 962,
                minor: None,
            },
            Self::Xsu => Meta {
                code: "XSU",
                numeric: 994,
                minor: None,
            },
            Self::Xts => Meta {
                code: "XTS",
                numeric: 963,
                minor: None,
            },
            Self::Xua => Meta {
                code: "XUA",
                numeric: 965,
                minor: None,
            },
            Self::Xxx => Meta {
                code: "XXX",
                numeric: 999,
                minor: None,
            },
            Self::Yer => Meta {
                code: "YER",
                numeric: 886,
                minor: Some(2),
            },
            Self::Zar => Meta {
                code: "ZAR",
                numeric: 710,
                minor: Some(2),
            },
            Self::Zmw => Meta {
                code: "ZMW",
                numeric: 967,
                minor: Some(2),
            },
            Self::Zwg => Meta {
                code: "ZWG",
                numeric: 924,
                minor: Some(2),
            },
        }
    }
}

/// The default currency is [`Xxx`](Currency::Xxx) — the currency-agnostic
/// identity, matching [`Money::ZERO`](crate::Money::ZERO) being `0 XXX`.
impl Default for Currency {
    fn default() -> Self {
        Self::Xxx
    }
}

/// Parses an ISO 4217 alphabetic code, **case-insensitively** — the round trip of
/// [`Display`](fmt::Display), and the door user input comes through (a CLI argument,
/// an environment variable, a query parameter).
///
/// # Case
///
/// `"USD"`, `"usd"` and `"Usd"` all parse to [`Usd`](Self::Usd), while
/// [`from_code`](Self::from_code) accepts only `"USD"`. The divergence is deliberate
/// and one-directional: every string `from_code` accepts, this accepts identically,
/// so the two never disagree about a *result* — only about which inputs to reject.
/// `from_code` stays the strict, canonical lookup (it is what the `serde` wire format
/// validates through, where a machine has no excuse for the wrong case), and
/// `FromStr` is the lenient one, because `"usd".parse()` failing is a papercut on
/// every human-facing surface (ADR-0061).
///
/// Leniency stops at case. There is no trimming, no numeric-code form, and no other
/// length: exactly three ASCII letters naming a known code.
///
/// # Errors
///
/// Returns [`TvmError::UnknownCurrencyCode`] for anything else. The error type is
/// [`TvmError`] rather than a dedicated one so a parse threads with `?` through the
/// same `Result<_, TvmError>` as the rest of the crate (ADR-0061).
///
/// ```
/// use time_value::{Currency, TvmError};
///
/// assert_eq!("USD".parse::<Currency>()?, Currency::Usd);
/// assert_eq!("jpy".parse::<Currency>()?, Currency::Jpy);
/// assert_eq!("Eur".parse::<Currency>()?, Currency::Eur);
///
/// // …and the strict lookup is unchanged.
/// assert_eq!(Currency::from_code("usd"), None);
///
/// for rejected in ["ZZZ", "US", "USDD", " USD", "840", ""] {
///     assert_eq!(rejected.parse::<Currency>(), Err(TvmError::UnknownCurrencyCode));
/// }
/// # Ok::<(), time_value::TvmError>(())
/// ```
impl core::str::FromStr for Currency {
    type Err = TvmError;

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        // Exactly three bytes, uppercased in place, then handed to the canonical
        // table — so `from_code` stays the single source of truth for which strings
        // name a currency. The slice pattern rejects every other length, including
        // the empty string, before any of that.
        let &[a, b, c] = code.as_bytes() else {
            return Err(TvmError::UnknownCurrencyCode);
        };
        let upper = [
            a.to_ascii_uppercase(),
            b.to_ascii_uppercase(),
            c.to_ascii_uppercase(),
        ];
        // ASCII-uppercasing touches only bytes below `0x80`, each of which is a
        // standalone UTF-8 code point, so `upper` is still valid UTF-8 and this arm
        // is unreachable. It is answered rather than unwrapped: the core is
        // panic-free by construction, not by a message nobody reads.
        core::str::from_utf8(&upper)
            .ok()
            .and_then(Self::from_code)
            .ok_or(TvmError::UnknownCurrencyCode)
    }
}

/// Formats as the ISO 4217 alphabetic code (`"USD"`).
impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::Currency;
    use crate::TvmError;

    // Compile-time tripwire against `Currency::ALL` drift (ADR-0045). `meta`
    // already forces every variant to carry metadata; this forces every variant
    // to be *named* here too. Adding a variant makes this match non-exhaustive,
    // so rustc fails the build until you name it — at which point this note sends
    // you to `Currency::ALL` and `EXPECTED_VARIANTS` below. It is a tripwire, not
    // a proof: stable Rust can't enumerate an enum's variants without a hand-list
    // or a proc-macro (`strum`), which the zero-dep core forgoes.
    #[allow(clippy::enum_glob_use)]
    fn _every_variant_is_named(c: Currency) {
        use Currency::*;
        match c {
            Aed | Afn | All | Amd | Aoa | Ars | Aud | Awg | Azn | Bam | Bbd | Bdt | Bhd | Bif
            | Bmd | Bnd | Bob | Bov | Brl | Bsd | Btn | Bwp | Byn | Bzd | Cad | Cdf | Che | Chf
            | Chw | Clf | Clp | Cny | Cop | Cou | Crc | Cup | Cve | Czk | Djf | Dkk | Dop | Dzd
            | Egp | Ern | Etb | Eur | Fjd | Fkp | Gbp | Gel | Ghs | Gip | Gmd | Gnf | Gtq | Gyd
            | Hkd | Hnl | Htg | Huf | Idr | Ils | Inr | Iqd | Irr | Isk | Jmd | Jod | Jpy | Kes
            | Kgs | Khr | Kmf | Kpw | Krw | Kwd | Kyd | Kzt | Lak | Lbp | Lkr | Lrd | Lsl | Lyd
            | Mad | Mdl | Mga | Mkd | Mmk | Mnt | Mop | Mru | Mur | Mvr | Mwk | Mxn | Mxv | Myr
            | Mzn | Nad | Ngn | Nio | Nok | Npr | Nzd | Omr | Pab | Pen | Pgk | Php | Pkr | Pln
            | Pyg | Qar | Ron | Rsd | Rub | Rwf | Sar | Sbd | Scr | Sdg | Sek | Sgd | Shp | Sle
            | Sos | Srd | Ssp | Stn | Svc | Syp | Szl | Thb | Tjs | Tmt | Tnd | Top | Try | Ttd
            | Twd | Tzs | Uah | Ugx | Usd | Uyi | Uyu | Uyw | Uzs | Ved | Ves | Vnd | Vuv | Wst
            | Xad | Xaf | Xag | Xau | Xba | Xbb | Xbc | Xbd | Xcd | Xcg | Xdr | Xof | Xpd | Xpf
            | Xpt | Xsu | Xts | Xua | Xxx | Yer | Zar | Zmw | Zwg => {}
        }
    }

    #[test]
    fn all_lists_every_variant_once() {
        // `_every_variant_is_named` forces awareness on the enum side; this pins
        // the `ALL` side. If a new variant is added to the enum but not to `ALL`,
        // the length check fails once `EXPECTED_VARIANTS` is bumped to match; and
        // no variant may appear twice.
        const EXPECTED_VARIANTS: usize = 177;
        assert_eq!(
            Currency::ALL.len(),
            EXPECTED_VARIANTS,
            "add the new variant to `ALL` and bump `EXPECTED_VARIANTS`"
        );
        for (i, &c) in Currency::ALL.iter().enumerate() {
            assert!(
                !Currency::ALL[..i].contains(&c),
                "{} listed twice in ALL",
                c.code()
            );
        }
    }

    /// The ordering policy the `Currency` rustdoc commits to (ADR-0053), pinned
    /// exhaustively over the finite enum (ADR-0045 rule 2). It fails the moment a
    /// new ISO code is *appended* rather than inserted alphabetically — which is
    /// the mistake `#[non_exhaustive]` makes look harmless.
    #[test]
    fn ord_is_alphabetical_by_iso_code() {
        for pair in Currency::ALL.windows(2) {
            let (earlier, later) = (pair[0], pair[1]);
            // `ALL` is itself declared in alphabetical order …
            assert!(
                earlier.code() < later.code(),
                "`ALL` is out of alphabetical order at {} / {}",
                earlier.code(),
                later.code()
            );
            // … and the derived `Ord` agrees with it, so `a < b` is `a.code() <
            // b.code()` for every pair, not just adjacent ones.
            assert!(
                earlier < later,
                "`Ord` disagrees with the code order at {} / {}",
                earlier.code(),
                later.code()
            );
        }
        // Non-adjacent pairs, across the whole set.
        for (i, &a) in Currency::ALL.iter().enumerate() {
            for &b in &Currency::ALL[i + 1..] {
                assert_eq!(
                    a.cmp(&b),
                    a.code().cmp(b.code()),
                    "{} vs {}",
                    a.code(),
                    b.code()
                );
            }
        }
    }

    #[test]
    fn code_round_trips_through_from_code() {
        for &c in Currency::ALL {
            assert_eq!(Currency::from_code(c.code()), Some(c), "{}", c.code());
        }
    }

    #[test]
    fn from_code_rejects_unknown() {
        assert_eq!(Currency::from_code("ZZZ"), None);
        assert_eq!(Currency::from_code("usd"), None); // case-sensitive
        assert_eq!(Currency::from_code(""), None);
        assert_eq!(Currency::from_code("US"), None);
    }

    /// `FromStr` round-trips [`Currency::code`] for **every** currency — the closed
    /// set is iterated, not sampled (ADR-0045 rule 2) — and does so in any casing,
    /// which is the leniency `from_code` does not have (ADR-0061).
    #[test]
    fn from_str_round_trips_every_code_in_any_case() {
        use core::str::FromStr as _;

        for &c in Currency::ALL {
            let upper = c.code();
            assert_eq!(upper.parse::<Currency>(), Ok(c), "{upper}");
            // The three casings a caller can produce without allocating: as ISO
            // writes it, all lower, and title case. `code()` is three ASCII letters,
            // so byte-wise casing is exact.
            let bytes = upper.as_bytes();
            let lower = [
                bytes[0].to_ascii_lowercase(),
                bytes[1].to_ascii_lowercase(),
                bytes[2].to_ascii_lowercase(),
            ];
            let title = [bytes[0], lower[1], lower[2]];
            for variant in [lower, title] {
                let s = core::str::from_utf8(&variant).unwrap();
                assert_eq!(s.parse::<Currency>(), Ok(c), "{s} should parse as {upper}");
            }
            // …and the parse agrees with the strict lookup wherever that one answers.
            assert_eq!(Currency::from_str(upper).ok(), Currency::from_code(upper));
        }
    }

    /// Everything `FromStr` rejects, and the one error it reports. Leniency stops at
    /// case: no trimming, no numeric code, no other length (ADR-0061).
    #[test]
    fn from_str_rejects_anything_that_is_not_a_code() {
        for rejected in [
            "", "U", "US", "USDD", " USD", "USD ", "U SD", "ZZZ", "zzz", "840", "\0\0\0",
            // Three *bytes* of valid UTF-8 that are not three ASCII letters: the
            // uppercasing leaves non-ASCII bytes alone, so the lookup simply misses.
            "€",
        ] {
            assert_eq!(
                rejected.parse::<Currency>(),
                Err(TvmError::UnknownCurrencyCode),
                "{rejected:?} should not parse"
            );
        }
    }

    /// The error's rendered message, since the variant exists to be reported
    /// (ADR-0052 pins `Display` for the same reason).
    #[test]
    #[cfg(feature = "alloc")]
    fn an_unknown_code_says_so_when_displayed() {
        use alloc::string::ToString as _;
        assert_eq!(
            "ZZZ".parse::<Currency>().unwrap_err().to_string(),
            "unknown ISO 4217 currency code"
        );
    }

    #[test]
    fn numeric_codes_are_unique() {
        let mut seen = [false; 1000];
        for &c in Currency::ALL {
            let n = c.numeric() as usize;
            assert!(!seen[n], "duplicate numeric {n}");
            seen[n] = true;
        }
    }

    #[test]
    fn minor_units_are_plausible() {
        for &c in Currency::ALL {
            if let Some(e) = c.minor_unit_exponent() {
                assert!(e <= 4, "{} has implausible minor unit {e}", c.code());
            }
        }
        assert_eq!(Currency::Xxx.minor_unit_exponent(), None);
        assert_eq!(Currency::Xau.minor_unit_exponent(), None);
    }

    #[test]
    fn default_is_xxx() {
        assert_eq!(Currency::default(), Currency::Xxx);
    }
}
