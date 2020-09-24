#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]

mod ext;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mock;

#[cfg(test)]
extern crate mocktopus;

#[cfg(test)]
use mocktopus::macros::mockable;

use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::traits::Currency;
/// # Exchange Rate Oracle implementation
/// This is the implementation of the Exchange Rate Oracle following the spec at:
/// https://interlay.gitlab.io/polkabtc-spec/spec/oracle.html
// Substrate
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure};
use frame_system::ensure_signed;
use sp_std::convert::TryInto;

pub(crate) type DOT<T> =
    <<T as collateral::Trait>::DOT as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

pub(crate) type PolkaBTC<T> =
    <<T as treasury::Trait>::PolkaBTC as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

/// ## Configuration and Constants
/// The pallet's configuration trait.
pub trait Trait:
    frame_system::Trait + timestamp::Trait + treasury::Trait + collateral::Trait + security::Trait
{
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

/// Granularity of exchange rate
pub const GRANULARITY: u128 = 5;

// This pallet's storage items.
decl_storage! {
    trait Store for Module<T: Trait> as ExchangeRateOracle {
        /// ## Storage
        /// Current BTC/DOT exchange rate
        ExchangeRate: u128;

        /// Last exchange rate time
        LastExchangeRateTime: T::Moment;

        /// Maximum delay for the exchange rate to be used
        MaxDelay: T::Moment;

        // Oracle allowed to set the exchange rate
        AuthorizedOracle get(fn admin) config(): T::AccountId;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        // Initializing events
        fn deposit_event() = default;

        #[weight = 1000]
        pub fn set_exchange_rate(origin, rate: u128) -> DispatchResult {
            // Check that Parachain is not in SHUTDOWN
            ext::security::ensure_parachain_status_not_shutdown::<T>()?;
            // Check that Parachain has not an INVALID error
            ext::security::ensure_parachain_error_not_invalid::<T>()?;


            let sender = ensure_signed(origin)?;

            // fail if the sender is not the authorized oracle
            ensure!(sender == Self::get_authorized_oracle(), Error::<T>::InvalidOracleSource);

            Self::_set_exchange_rate(rate)?;

            Self::deposit_event(Event::<T>::SetExchangeRate(sender, rate));

            Ok(())
        }
    }
}

#[cfg_attr(test, mockable)]
impl<T: Trait> Module<T> {
    /// Public getters
    pub fn get_exchange_rate() -> Result<u128, DispatchError> {
        let max_delay_passed = Self::is_max_delay_passed()?;
        ensure!(!max_delay_passed, Error::<T>::MissingExchangeRate);
        Ok(<ExchangeRate>::get())
    }

    pub fn btc_to_u128(amount: PolkaBTC<T>) -> Result<u128, DispatchError> {
        Self::into_u128(amount)
    }

    pub fn dot_to_u128(amount: DOT<T>) -> Result<u128, DispatchError> {
        Self::into_u128(amount)
    }

    fn into_u128<I: TryInto<u128>>(x: I) -> Result<u128, DispatchError> {
        TryInto::<u128>::try_into(x).map_err(|_e| Error::<T>::ConversionError.into())
    }

    pub fn btc_to_dots(amount: PolkaBTC<T>) -> Result<DOT<T>, DispatchError> {
        let rate = Self::get_exchange_rate()?;
        let raw_amount = Self::into_u128(amount)?;
        let converted = rate
            .checked_mul(raw_amount)
            .ok_or(Error::<T>::ConversionError)?;
        let result = converted
            .try_into()
            .map_err(|_e| Error::<T>::ConversionError)?;
        Ok(result)
    }

    pub fn dots_to_btc(amount: DOT<T>) -> Result<PolkaBTC<T>, DispatchError> {
        let rate = Self::get_exchange_rate()?;
        let raw_amount = Self::into_u128(amount)?;
        if raw_amount == 0 {
            return Ok(0.into());
        }
        let converted = raw_amount
            .checked_div(rate)
            .ok_or(Error::<T>::ConversionError)?;
        let result = converted
            .try_into()
            .map_err(|_e| Error::<T>::ConversionError)?;
        Ok(result)
    }

    pub fn get_last_exchange_rate_time() -> T::Moment {
        <LastExchangeRateTime<T>>::get()
    }

    /// Private getters and setters
    fn get_max_delay() -> T::Moment {
        <MaxDelay<T>>::get()
    }

    pub fn _set_exchange_rate(rate: u128) -> DispatchResult {
        Self::set_current_rate(rate);
        // recover if the max delay was already passed
        if Self::is_max_delay_passed()? {
            Self::recover_from_oracle_offline()?;
        }
        let now = Self::get_current_time();
        Self::set_last_exchange_rate_time(now);
        Ok(())
    }

    pub fn set_current_rate(rate: u128) {
        <ExchangeRate>::put(rate);
    }

    fn set_last_exchange_rate_time(time: T::Moment) {
        <LastExchangeRateTime<T>>::put(time);
    }

    fn get_authorized_oracle() -> T::AccountId {
        <AuthorizedOracle<T>>::get()
    }

    fn recover_from_oracle_offline() -> DispatchResult {
        ext::security::recover_from_oracle_offline::<T>()
    }

    /// Returns true if the last update to the exchange rate
    /// was before the maximum allowed delay
    pub fn is_max_delay_passed() -> Result<bool, DispatchError> {
        let timestamp = Self::get_current_time();
        let last_update = Self::get_last_exchange_rate_time();
        let max_delay = Self::get_max_delay();
        Ok(timestamp - last_update > max_delay)
    }

    /// Returns the current timestamp
    fn get_current_time() -> T::Moment {
        <timestamp::Module<T>>::get()
    }
}

decl_event! {
    /// ## Events
    pub enum Event<T> where
            AccountId = <T as frame_system::Trait>::AccountId {
        /// Event emitted when exchange rate is set
        SetExchangeRate(AccountId, u128),
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Not authorized to set exchange rate
        InvalidOracleSource,
        /// Exchange rate not specified or has expired
        MissingExchangeRate,
        /// Failed to convert currency
        ConversionError,
    }
}
