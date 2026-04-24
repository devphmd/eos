//! # eos-rs
//!
//! Safe(ish) Rust wrappers around the Epic Online Services (EOS) C SDK.
//!
//! This crate is designed as an ergonomic layer over [`eos_sys`], while still
//! allowing escape hatches to raw handles for APIs that are not yet wrapped.
//!
//! ## Chapter 1: Installation
//!
//! Add dependency:
//!
//! ```toml
//! [dependencies]
//! eos-rs = "0.1"
//! ```
//!
//! Provide EOS SDK location when building:
//!
//! - `EOS_SDK_DIR=/path/to/EOS-SDK`
//!
//! Expected layout inside `EOS_SDK_DIR`:
//!
//! - `Include/`
//! - `Lib/`
//!
//! On Windows, ship `EOSSDK-Win64-Shipping.dll` with your app.
//!
//! ## Chapter 2: SDK Lifecycle
//!
//! EOS has a global lifecycle:
//!
//! 1. [`initialize`]
//! 2. Create a [`Platform`]
//! 3. Call [`Platform::tick`] regularly
//! 4. Drop `Platform`
//! 5. [`shutdown`]
//!
//! ## Chapter 3: Quick Start
//!
//! ```no_run
//! use eos_rs::{initialize, shutdown, InitializeOptions, Platform, PlatformOptions};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     initialize(InitializeOptions {
//!         product_name: "my-game".to_string(),
//!         product_version: "0.1.0".to_string(),
//!     })?;
//!
//!     let platform = Platform::create(PlatformOptions {
//!         product_id: "product-id".to_string(),
//!         sandbox_id: "sandbox-id".to_string(),
//!         deployment_id: "deployment-id".to_string(),
//!         client_id: "client-id".to_string(),
//!         client_secret: "client-secret".to_string(),
//!         is_server: false,
//!         encryption_key: None,
//!         override_country_code: None,
//!         override_locale_code: None,
//!     })?;
//!
//!     // Call once per frame/tick in your game loop.
//!     platform.tick();
//!
//!     drop(platform);
//!     shutdown()?;
//!     Ok(())
//! }
//! ```
//!
//! ## Chapter 4: Interfaces and Raw Escape Hatch
//!
//! [`Platform`] exposes typed accessors for EOS interfaces (`auth()`, `connect()`,
//! `lobby()`, `p2p()`, and more). Each wrapper provides `raw_handle()` so you can
//! call any function from [`sys`] directly when needed.
//!
//! ## Chapter 5: Callback Model
//!
//! EOS async APIs are callback-based. `eos-rs` wraps selected callbacks with Rust
//! closures and owns callback context allocations until EOS invokes them.
//!
//! ## Chapter 6: Owned EOS Objects
//!
//! EOS returns many heap/handle values that require `*_Release`. This crate defines
//! RAII wrappers for those objects; dropping the wrapper calls the matching release
//! API automatically.
//!
//! ## Chapter 7: Safety Notes
//!
//! This crate significantly improves safety over raw FFI, but it is not fully safe:
//!
//! - EOS threading requirements still apply.
//! - EOS callback contracts still apply.
//! - `raw_handle()` access can bypass invariants.
//!
//! Prefer wrapped methods and RAII types where available.
//!
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::ptr::{null, null_mut, NonNull};
use std::sync::OnceLock;

pub use eos_sys as sys;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("EOS error: {0:?}")]
    Eos(sys::EOS_EResult),
    #[error("nul byte in string")]
    Nul(#[from] std::ffi::NulError),
    #[error("null pointer from EOS SDK")]
    Null,
}

pub type Result<T> = std::result::Result<T, Error>;

fn ok(res: sys::EOS_EResult) -> Result<()> {
    if res == sys::EOS_EResult_EOS_Success {
        Ok(())
    } else {
        Err(Error::Eos(res))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginStatus {
    NotLoggedIn,
    UsingLocalProfile,
    LoggedIn,
    Other(sys::EOS_ELoginStatus),
}

impl LoginStatus {
    fn from_raw(status: sys::EOS_ELoginStatus) -> Self {
        match status {
            x if x == sys::EOS_ELoginStatus_EOS_LS_NotLoggedIn => Self::NotLoggedIn,
            x if x == sys::EOS_ELoginStatus_EOS_LS_UsingLocalProfile => Self::UsingLocalProfile,
            x if x == sys::EOS_ELoginStatus_EOS_LS_LoggedIn => Self::LoggedIn,
            x => Self::Other(x),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NatType {
    Unknown,
    Open,
    Moderate,
    Strict,
    Other(sys::EOS_ENATType),
}

impl NatType {
    fn from_raw(v: sys::EOS_ENATType) -> Self {
        match v {
            x if x == sys::EOS_ENATType_EOS_NAT_Unknown => Self::Unknown,
            x if x == sys::EOS_ENATType_EOS_NAT_Open => Self::Open,
            x if x == sys::EOS_ENATType_EOS_NAT_Moderate => Self::Moderate,
            x if x == sys::EOS_ENATType_EOS_NAT_Strict => Self::Strict,
            x => Self::Other(x),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayControl {
    NoRelays,
    AllowRelays,
    ForceRelays,
    Other(sys::EOS_ERelayControl),
}

impl RelayControl {
    fn from_raw(v: sys::EOS_ERelayControl) -> Self {
        match v {
            x if x == sys::EOS_ERelayControl_EOS_RC_NoRelays => Self::NoRelays,
            x if x == sys::EOS_ERelayControl_EOS_RC_AllowRelays => Self::AllowRelays,
            x if x == sys::EOS_ERelayControl_EOS_RC_ForceRelays => Self::ForceRelays,
            x => Self::Other(x),
        }
    }

    fn to_raw(self) -> sys::EOS_ERelayControl {
        match self {
            Self::NoRelays => sys::EOS_ERelayControl_EOS_RC_NoRelays,
            Self::AllowRelays => sys::EOS_ERelayControl_EOS_RC_AllowRelays,
            Self::ForceRelays => sys::EOS_ERelayControl_EOS_RC_ForceRelays,
            Self::Other(v) => v,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketReliability {
    UnreliableUnordered,
    ReliableUnordered,
    ReliableOrdered,
}

impl PacketReliability {
    fn to_raw(self) -> sys::EOS_EPacketReliability {
        match self {
            Self::UnreliableUnordered => sys::EOS_EPacketReliability_EOS_PR_UnreliableUnordered,
            Self::ReliableUnordered => sys::EOS_EPacketReliability_EOS_PR_ReliableUnordered,
            Self::ReliableOrdered => sys::EOS_EPacketReliability_EOS_PR_ReliableOrdered,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PacketQueueInfo {
    pub incoming_max_size_bytes: u64,
    pub incoming_current_size_bytes: u64,
    pub incoming_current_packet_count: u64,
    pub outgoing_max_size_bytes: u64,
    pub outgoing_current_size_bytes: u64,
    pub outgoing_current_packet_count: u64,
}

#[derive(Clone, Debug)]
pub enum LobbySearchValue {
    Bool(bool),
    Int64(i64),
    Double(f64),
    String(String),
}

#[derive(Clone, Debug)]
pub struct ReceivedPacket {
    pub peer_id: ProductUserId,
    pub socket_name: String,
    pub channel: u8,
    pub data: Vec<u8>,
}

pub fn result_to_string(result: sys::EOS_EResult) -> &'static str {
    // SAFETY: EOS guarantees this pointer is non-null, static, and valid UTF-8-ish C string.
    let ptr = unsafe { sys::EOS_EResult_ToString(result) };
    if ptr.is_null() {
        return "EOS_Unknown";
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("EOS_InvalidUtf8")
}

fn make_socket_id(socket_name: &str) -> Result<sys::EOS_P2P_SocketId> {
    let c = CString::new(socket_name)?;
    let bytes = c.as_bytes_with_nul();
    if bytes.len() > sys::EOS_P2P_SOCKETID_SOCKETNAME_SIZE as usize {
        return Err(Error::Null);
    }
    let mut out = sys::EOS_P2P_SocketId {
        ApiVersion: sys::EOS_P2P_SOCKETID_API_LATEST as i32,
        SocketName: [0; sys::EOS_P2P_SOCKETID_SOCKETNAME_SIZE as usize],
    };
    for (idx, b) in bytes.iter().copied().enumerate() {
        out.SocketName[idx] = b as i8;
    }
    Ok(out)
}

fn socket_name_from_raw(socket: &sys::EOS_P2P_SocketId) -> String {
    let ptr = socket.SocketName.as_ptr();
    unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
}

static INITIALIZED: OnceLock<()> = OnceLock::new();

pub struct InitializeOptions {
    pub product_name: String,
    pub product_version: String,
}

pub fn initialize(opts: InitializeOptions) -> Result<()> {
    if INITIALIZED.get().is_some() {
        return Ok(());
    }

    let product_name = CString::new(opts.product_name)?;
    let product_version = CString::new(opts.product_version)?;

    let init_opts = sys::EOS_InitializeOptions {
        ApiVersion: sys::EOS_INITIALIZE_API_LATEST as i32,
        AllocateMemoryFunction: None,
        ReallocateMemoryFunction: None,
        ReleaseMemoryFunction: None,
        ProductName: product_name.as_ptr(),
        ProductVersion: product_version.as_ptr(),
        Reserved: null_mut(),
        SystemInitializeOptions: null_mut(),
        OverrideThreadAffinity: null_mut(),
    };

    // SAFETY: EOS expects pointers valid for the duration of the call.
    let res = unsafe { sys::EOS_Initialize(&init_opts) };
    ok(res)?;
    let _ = INITIALIZED.set(());
    Ok(())
}

pub fn shutdown() -> Result<()> {
    if INITIALIZED.get().is_none() {
        return Ok(());
    }
    let res = unsafe { sys::EOS_Shutdown() };
    ok(res)
}

#[derive(Clone, Copy, Debug)]
pub struct EpicAccountId(sys::EOS_EpicAccountId);

#[derive(Clone, Copy, Debug)]
pub struct ProductUserId(sys::EOS_ProductUserId);

#[derive(Clone, Copy, Debug)]
pub struct ContinuanceToken(sys::EOS_ContinuanceToken);

impl ContinuanceToken {
    pub fn raw(self) -> sys::EOS_ContinuanceToken {
        self.0
    }

    pub fn from_login_callback(info: &sys::EOS_Connect_LoginCallbackInfo) -> Option<Self> {
        if info.ContinuanceToken.is_null() {
            None
        } else {
            Some(Self(info.ContinuanceToken))
        }
    }
}

#[derive(Clone, Debug)]
pub struct CreateLobbyParams {
    pub max_lobby_members: u32,
    pub permission_level: sys::EOS_ELobbyPermissionLevel,
    pub presence_enabled: bool,
    pub allow_invites: bool,
    pub bucket_id: String,
    pub disable_host_migration: bool,
    pub enable_rtc_room: bool,
    pub enable_join_by_id: bool,
    pub rejoin_after_kick_requires_invite: bool,
}

impl Default for CreateLobbyParams {
    fn default() -> Self {
        Self {
            max_lobby_members: 8,
            permission_level: sys::EOS_ELobbyPermissionLevel_EOS_LPL_PUBLICADVERTISED,
            presence_enabled: true,
            allow_invites: true,
            bucket_id: "default".to_string(),
            disable_host_migration: false,
            enable_rtc_room: false,
            enable_join_by_id: false,
            rejoin_after_kick_requires_invite: false,
        }
    }
}

impl EpicAccountId {
    pub fn from_string(s: &str) -> Result<Self> {
        let s = CString::new(s)?;
        let raw = unsafe { sys::EOS_EpicAccountId_FromString(s.as_ptr()) };
        let id = Self(raw);
        if id.is_valid() {
            Ok(id)
        } else {
            Err(Error::Null)
        }
    }

    pub fn to_string(self) -> Result<String> {
        let mut buf = vec![0i8; (sys::EOS_EPICACCOUNTID_MAX_LENGTH + 1) as usize];
        let mut len = buf.len() as i32;
        let res = unsafe { sys::EOS_EpicAccountId_ToString(self.0, buf.as_mut_ptr(), &mut len) };
        ok(res)?;
        let s = unsafe { CStr::from_ptr(buf.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        Ok(s)
    }

    pub fn is_valid(self) -> bool {
        unsafe { sys::EOS_EpicAccountId_IsValid(self.0) != 0 }
    }

    pub fn raw(self) -> sys::EOS_EpicAccountId {
        self.0
    }
}

impl ProductUserId {
    pub fn from_string(s: &str) -> Result<Self> {
        let s = CString::new(s)?;
        let raw = unsafe { sys::EOS_ProductUserId_FromString(s.as_ptr()) };
        let id = Self(raw);
        if id.is_valid() {
            Ok(id)
        } else {
            Err(Error::Null)
        }
    }

    pub fn to_string(self) -> Result<String> {
        let mut buf = vec![0i8; (sys::EOS_PRODUCTUSERID_MAX_LENGTH + 1) as usize];
        let mut len = buf.len() as i32;
        let res = unsafe { sys::EOS_ProductUserId_ToString(self.0, buf.as_mut_ptr(), &mut len) };
        ok(res)?;
        let s = unsafe { CStr::from_ptr(buf.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        Ok(s)
    }

    pub fn is_valid(self) -> bool {
        unsafe { sys::EOS_ProductUserId_IsValid(self.0) != 0 }
    }

    pub fn raw(self) -> sys::EOS_ProductUserId {
        self.0
    }
}

pub struct Platform(NonNull<sys::EOS_PlatformHandle>);

unsafe impl Send for Platform {}
unsafe impl Sync for Platform {}

pub struct PlatformOptions {
    pub product_id: String,
    pub sandbox_id: String,
    pub deployment_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub is_server: bool,
    pub encryption_key: Option<String>,
    pub override_country_code: Option<String>,
    pub override_locale_code: Option<String>,
}

impl Platform {
    pub fn create(opts: PlatformOptions) -> Result<Self> {
        let product_id = CString::new(opts.product_id)?;
        let sandbox_id = CString::new(opts.sandbox_id)?;
        let deployment_id = CString::new(opts.deployment_id)?;
        let client_id = CString::new(opts.client_id)?;
        let client_secret = CString::new(opts.client_secret)?;
        let encryption_key = match opts.encryption_key {
            Some(s) => Some(CString::new(s)?),
            None => None,
        };
        let override_country_code = match opts.override_country_code {
            Some(s) => Some(CString::new(s)?),
            None => None,
        };
        let override_locale_code = match opts.override_locale_code {
            Some(s) => Some(CString::new(s)?),
            None => None,
        };

        let options = sys::EOS_Platform_Options {
            ApiVersion: sys::EOS_PLATFORM_OPTIONS_API_LATEST as i32,
            Reserved: null_mut(),
            bIsServer: if opts.is_server { 1 } else { 0 },
            EncryptionKey: encryption_key.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
            OverrideCountryCode: override_country_code
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(std::ptr::null()),
            OverrideLocaleCode: override_locale_code
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(std::ptr::null()),
            ProductId: product_id.as_ptr(),
            SandboxId: sandbox_id.as_ptr(),
            DeploymentId: deployment_id.as_ptr(),
            ClientCredentials: sys::EOS_Platform_ClientCredentials {
                ClientId: client_id.as_ptr(),
                ClientSecret: client_secret.as_ptr(),
            },
            Flags: 0,
            CacheDirectory: std::ptr::null(),
            TickBudgetInMilliseconds: 0,
            RTCOptions: std::ptr::null(),
            IntegratedPlatformOptionsContainerHandle: null_mut(),
            SystemSpecificOptions: null(),
            TaskNetworkTimeoutSeconds: null_mut(),
        };

        let handle = unsafe { sys::EOS_Platform_Create(&options) };
        let nn = NonNull::new(handle as *mut sys::EOS_PlatformHandle).ok_or(Error::Null)?;
        Ok(Self(nn))
    }

    #[inline]
    fn as_handle(&self) -> sys::EOS_HPlatform {
        self.0.as_ptr() as sys::EOS_HPlatform
    }

    pub fn tick(&self) {
        unsafe { sys::EOS_Platform_Tick(self.as_handle()) };
    }

    pub fn raw_handle(&self) -> sys::EOS_HPlatform {
        self.as_handle()
    }

    pub fn auth(&self) -> Auth {
        let h = unsafe { sys::EOS_Platform_GetAuthInterface(self.as_handle()) };
        Auth(NonNull::new(h as *mut sys::EOS_AuthHandle).expect("EOS auth interface null"))
    }

    pub fn connect(&self) -> Connect {
        let h = unsafe { sys::EOS_Platform_GetConnectInterface(self.as_handle()) };
        Connect(NonNull::new(h as *mut sys::EOS_ConnectHandle).expect("EOS connect interface null"))
    }

    pub fn achievements(&self) -> Achievements {
        let h = unsafe { sys::EOS_Platform_GetAchievementsInterface(self.as_handle()) };
        Achievements(
            NonNull::new(h as *mut sys::EOS_AchievementsHandle)
                .expect("EOS achievements interface null"),
        )
    }

    pub fn anticheat_client(&self) -> AntiCheatClient {
        let h = unsafe { sys::EOS_Platform_GetAntiCheatClientInterface(self.as_handle()) };
        AntiCheatClient(
            NonNull::new(h as *mut sys::EOS_AntiCheatClientHandle)
                .expect("EOS anticheat client interface null"),
        )
    }

    pub fn anticheat_server(&self) -> AntiCheatServer {
        let h = unsafe { sys::EOS_Platform_GetAntiCheatServerInterface(self.as_handle()) };
        AntiCheatServer(
            NonNull::new(h as *mut sys::EOS_AntiCheatServerHandle)
                .expect("EOS anticheat server interface null"),
        )
    }

    pub fn custom_invites(&self) -> CustomInvites {
        let h = unsafe { sys::EOS_Platform_GetCustomInvitesInterface(self.as_handle()) };
        CustomInvites(
            NonNull::new(h as *mut sys::EOS_CustomInvitesHandle)
                .expect("EOS custom invites interface null"),
        )
    }

    pub fn ecom(&self) -> Ecom {
        let h = unsafe { sys::EOS_Platform_GetEcomInterface(self.as_handle()) };
        Ecom(NonNull::new(h as *mut sys::EOS_EcomHandle).expect("EOS ecom interface null"))
    }

    pub fn friends(&self) -> Friends {
        let h = unsafe { sys::EOS_Platform_GetFriendsInterface(self.as_handle()) };
        Friends(NonNull::new(h as *mut sys::EOS_FriendsHandle).expect("EOS friends interface null"))
    }

    pub fn integrated_platform(&self) -> IntegratedPlatform {
        let h = unsafe { sys::EOS_Platform_GetIntegratedPlatformInterface(self.as_handle()) };
        IntegratedPlatform(
            NonNull::new(h as *mut sys::EOS_IntegratedPlatformHandle)
                .expect("EOS integrated platform interface null"),
        )
    }

    pub fn kws(&self) -> Kws {
        let h = unsafe { sys::EOS_Platform_GetKWSInterface(self.as_handle()) };
        Kws(NonNull::new(h as *mut sys::EOS_KWSHandle).expect("EOS KWS interface null"))
    }

    pub fn leaderboards(&self) -> Leaderboards {
        let h = unsafe { sys::EOS_Platform_GetLeaderboardsInterface(self.as_handle()) };
        Leaderboards(
            NonNull::new(h as *mut sys::EOS_LeaderboardsHandle)
                .expect("EOS leaderboards interface null"),
        )
    }

    pub fn lobby(&self) -> Lobby {
        let h = unsafe { sys::EOS_Platform_GetLobbyInterface(self.as_handle()) };
        Lobby(NonNull::new(h as *mut sys::EOS_LobbyHandle).expect("EOS lobby interface null"))
    }

    pub fn metrics(&self) -> Metrics {
        let h = unsafe { sys::EOS_Platform_GetMetricsInterface(self.as_handle()) };
        Metrics(NonNull::new(h as *mut sys::EOS_MetricsHandle).expect("EOS metrics interface null"))
    }

    pub fn mods(&self) -> Mods {
        let h = unsafe { sys::EOS_Platform_GetModsInterface(self.as_handle()) };
        Mods(NonNull::new(h as *mut sys::EOS_ModsHandle).expect("EOS mods interface null"))
    }

    pub fn p2p(&self) -> P2P {
        let h = unsafe { sys::EOS_Platform_GetP2PInterface(self.as_handle()) };
        P2P(NonNull::new(h as *mut sys::EOS_P2PHandle).expect("EOS p2p interface null"))
    }

    pub fn player_data_storage(&self) -> PlayerDataStorage {
        let h = unsafe { sys::EOS_Platform_GetPlayerDataStorageInterface(self.as_handle()) };
        PlayerDataStorage(
            NonNull::new(h as *mut sys::EOS_PlayerDataStorageHandle)
                .expect("EOS player data storage interface null"),
        )
    }

    pub fn presence(&self) -> Presence {
        let h = unsafe { sys::EOS_Platform_GetPresenceInterface(self.as_handle()) };
        Presence(
            NonNull::new(h as *mut sys::EOS_PresenceHandle).expect("EOS presence interface null"),
        )
    }

    pub fn progressionsnapshot(&self) -> ProgressionSnapshot {
        let h = unsafe { sys::EOS_Platform_GetProgressionSnapshotInterface(self.as_handle()) };
        ProgressionSnapshot(
            NonNull::new(h as *mut sys::EOS_ProgressionSnapshotHandle)
                .expect("EOS progression snapshot interface null"),
        )
    }

    pub fn reports(&self) -> Reports {
        let h = unsafe { sys::EOS_Platform_GetReportsInterface(self.as_handle()) };
        Reports(
            NonNull::new(h as *mut sys::EOS_ReportsHandle).expect("EOS reports interface null"),
        )
    }

    pub fn rtc(&self) -> Rtc {
        let h = unsafe { sys::EOS_Platform_GetRTCInterface(self.as_handle()) };
        Rtc(NonNull::new(h as *mut sys::EOS_RTCHandle).expect("EOS RTC interface null"))
    }

    pub fn rtc_admin(&self) -> RtcAdmin {
        let h = unsafe { sys::EOS_Platform_GetRTCAdminInterface(self.as_handle()) };
        RtcAdmin(
            NonNull::new(h as *mut sys::EOS_RTCAdminHandle).expect("EOS RTC admin interface null"),
        )
    }

    pub fn sanctions(&self) -> Sanctions {
        let h = unsafe { sys::EOS_Platform_GetSanctionsInterface(self.as_handle()) };
        Sanctions(
            NonNull::new(h as *mut sys::EOS_SanctionsHandle).expect("EOS sanctions interface null"),
        )
    }

    pub fn sessions(&self) -> Sessions {
        let h = unsafe { sys::EOS_Platform_GetSessionsInterface(self.as_handle()) };
        Sessions(
            NonNull::new(h as *mut sys::EOS_SessionsHandle).expect("EOS sessions interface null"),
        )
    }

    pub fn stats(&self) -> Stats {
        let h = unsafe { sys::EOS_Platform_GetStatsInterface(self.as_handle()) };
        Stats(NonNull::new(h as *mut sys::EOS_StatsHandle).expect("EOS stats interface null"))
    }

    pub fn title_storage(&self) -> TitleStorage {
        let h = unsafe { sys::EOS_Platform_GetTitleStorageInterface(self.as_handle()) };
        TitleStorage(
            NonNull::new(h as *mut sys::EOS_TitleStorageHandle)
                .expect("EOS title storage interface null"),
        )
    }

    pub fn ui(&self) -> Ui {
        let h = unsafe { sys::EOS_Platform_GetUIInterface(self.as_handle()) };
        Ui(NonNull::new(h as *mut sys::EOS_UIHandle).expect("EOS UI interface null"))
    }

    pub fn userinfo(&self) -> UserInfo {
        let h = unsafe { sys::EOS_Platform_GetUserInfoInterface(self.as_handle()) };
        UserInfo(
            NonNull::new(h as *mut sys::EOS_UserInfoHandle).expect("EOS userinfo interface null"),
        )
    }
}

impl Drop for Platform {
    fn drop(&mut self) {
        unsafe { sys::EOS_Platform_Release(self.as_handle()) };
    }
}

pub struct Auth(NonNull<sys::EOS_AuthHandle>);
pub struct Connect(NonNull<sys::EOS_ConnectHandle>);
pub struct Achievements(NonNull<sys::EOS_AchievementsHandle>);
pub struct AntiCheatClient(NonNull<sys::EOS_AntiCheatClientHandle>);
pub struct AntiCheatServer(NonNull<sys::EOS_AntiCheatServerHandle>);
pub struct CustomInvites(NonNull<sys::EOS_CustomInvitesHandle>);
pub struct Ecom(NonNull<sys::EOS_EcomHandle>);
pub struct Friends(NonNull<sys::EOS_FriendsHandle>);
pub struct IntegratedPlatform(NonNull<sys::EOS_IntegratedPlatformHandle>);
pub struct Kws(NonNull<sys::EOS_KWSHandle>);
pub struct Leaderboards(NonNull<sys::EOS_LeaderboardsHandle>);
pub struct Lobby(NonNull<sys::EOS_LobbyHandle>);
pub struct Metrics(NonNull<sys::EOS_MetricsHandle>);
pub struct Mods(NonNull<sys::EOS_ModsHandle>);
pub struct P2P(NonNull<sys::EOS_P2PHandle>);
pub struct PlayerDataStorage(NonNull<sys::EOS_PlayerDataStorageHandle>);
pub struct Presence(NonNull<sys::EOS_PresenceHandle>);
pub struct ProgressionSnapshot(NonNull<sys::EOS_ProgressionSnapshotHandle>);
pub struct Reports(NonNull<sys::EOS_ReportsHandle>);
pub struct Rtc(NonNull<sys::EOS_RTCHandle>);
pub struct RtcAdmin(NonNull<sys::EOS_RTCAdminHandle>);
pub struct Sanctions(NonNull<sys::EOS_SanctionsHandle>);
pub struct Sessions(NonNull<sys::EOS_SessionsHandle>);
pub struct Stats(NonNull<sys::EOS_StatsHandle>);
pub struct TitleStorage(NonNull<sys::EOS_TitleStorageHandle>);
pub struct Ui(NonNull<sys::EOS_UIHandle>);
pub struct UserInfo(NonNull<sys::EOS_UserInfoHandle>);

/// Owns a callback allocation until EOS triggers it once.
struct CallbackOnce<T> {
    ptr: *mut T,
    _marker: PhantomData<T>,
}

impl<T> CallbackOnce<T> {
    fn new(val: T) -> Self {
        let b = Box::new(val);
        Self {
            ptr: Box::into_raw(b),
            _marker: PhantomData,
        }
    }
}

impl Auth {
    fn as_handle(&self) -> sys::EOS_HAuth {
        self.0.as_ptr() as sys::EOS_HAuth
    }

    pub fn raw_handle(&self) -> sys::EOS_HAuth {
        self.as_handle()
    }

    pub fn get_login_status(&self, local_user: EpicAccountId) -> LoginStatus {
        let raw = unsafe { sys::EOS_Auth_GetLoginStatus(self.as_handle(), local_user.raw()) };
        LoginStatus::from_raw(raw)
    }

    pub fn copy_user_auth_token(&self, local_user: EpicAccountId) -> Result<AuthToken> {
        let options = sys::EOS_Auth_CopyUserAuthTokenOptions {
            ApiVersion: sys::EOS_AUTH_COPYUSERAUTHTOKEN_API_LATEST as i32,
        };
        let mut token_ptr: *mut sys::EOS_Auth_Token = std::ptr::null_mut();
        let res = unsafe {
            sys::EOS_Auth_CopyUserAuthToken(
                self.as_handle(),
                &options,
                local_user.raw(),
                &mut token_ptr,
            )
        };
        ok(res)?;
        unsafe { AuthToken::from_raw(token_ptr) }
    }

    pub fn copy_id_token(&self, account: EpicAccountId) -> Result<AuthIdToken> {
        let options = sys::EOS_Auth_CopyIdTokenOptions {
            ApiVersion: sys::EOS_AUTH_COPYIDTOKEN_API_LATEST as i32,
            AccountId: account.raw(),
        };
        let mut token_ptr: *mut sys::EOS_Auth_IdToken = std::ptr::null_mut();
        let res = unsafe { sys::EOS_Auth_CopyIdToken(self.as_handle(), &options, &mut token_ptr) };
        ok(res)?;
        unsafe { AuthIdToken::from_raw(token_ptr) }
    }

    pub fn query_id_token(
        &self,
        local_user: EpicAccountId,
        target_account: EpicAccountId,
        cb: impl FnOnce(Result<sys::EOS_Auth_QueryIdTokenCallbackInfo>) + Send + 'static,
    ) {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Auth_QueryIdTokenCallbackInfo>) + Send>>,
        }

        unsafe extern "C" fn trampoline(data: *const sys::EOS_Auth_QueryIdTokenCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }

        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });

        let options = sys::EOS_Auth_QueryIdTokenOptions {
            ApiVersion: sys::EOS_AUTH_QUERYIDTOKEN_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            TargetAccountId: target_account.raw(),
        };

        unsafe {
            sys::EOS_Auth_QueryIdToken(
                self.as_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
    }

    pub fn logout(
        &self,
        local_user: EpicAccountId,
        cb: impl FnOnce(Result<sys::EOS_Auth_LogoutCallbackInfo>) + Send + 'static,
    ) {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Auth_LogoutCallbackInfo>) + Send>>,
        }

        unsafe extern "C" fn trampoline(data: *const sys::EOS_Auth_LogoutCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }

        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });

        let options = sys::EOS_Auth_LogoutOptions {
            ApiVersion: sys::EOS_AUTH_LOGOUT_API_LATEST as i32,
            LocalUserId: local_user.raw(),
        };

        unsafe {
            sys::EOS_Auth_Logout(
                self.as_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
    }

    pub fn login_epic_exchange_code(
        &self,
        exchange_code: &str,
        cb: impl FnOnce(Result<sys::EOS_Auth_LoginCallbackInfo>) + Send + 'static,
    ) -> Result<()> {
        let exchange_code = CString::new(exchange_code)?;

        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Auth_LoginCallbackInfo>) + Send>>,
        }

        unsafe extern "C" fn trampoline(data: *const sys::EOS_Auth_LoginCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }

        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });

        let creds = sys::EOS_Auth_Credentials {
            ApiVersion: sys::EOS_AUTH_CREDENTIALS_API_LATEST as i32,
            Id: std::ptr::null(),
            Token: exchange_code.as_ptr(),
            Type: sys::EOS_ELoginCredentialType_EOS_LCT_ExchangeCode,
            SystemAuthCredentialsOptions: null_mut(),
            ExternalType: sys::EOS_EExternalCredentialType_EOS_ECT_EPIC,
        };

        let options = sys::EOS_Auth_LoginOptions {
            ApiVersion: sys::EOS_AUTH_LOGIN_API_LATEST as i32,
            Credentials: &creds,
            ScopeFlags: 0,
            LoginFlags: 0,
        };

        unsafe {
            sys::EOS_Auth_Login(
                self.as_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
        Ok(())
    }
}

impl Connect {
    pub fn raw_handle(&self) -> sys::EOS_HConnect {
        self.0.as_ptr() as sys::EOS_HConnect
    }

    pub fn get_login_status(&self, local_user: ProductUserId) -> LoginStatus {
        let raw = unsafe { sys::EOS_Connect_GetLoginStatus(self.raw_handle(), local_user.raw()) };
        LoginStatus::from_raw(raw)
    }

    pub fn copy_id_token(&self, local_user: ProductUserId) -> Result<ConnectIdToken> {
        let options = sys::EOS_Connect_CopyIdTokenOptions {
            ApiVersion: sys::EOS_CONNECT_COPYIDTOKEN_API_LATEST as i32,
            LocalUserId: local_user.raw(),
        };
        let mut token_ptr: *mut sys::EOS_Connect_IdToken = std::ptr::null_mut();
        let res = unsafe { sys::EOS_Connect_CopyIdToken(self.raw_handle(), &options, &mut token_ptr) };
        ok(res)?;
        unsafe { ConnectIdToken::from_raw(token_ptr) }
    }

    pub fn login_openid_access_token(
        &self,
        token: &str,
        display_name: Option<&str>,
        cb: impl FnOnce(Result<sys::EOS_Connect_LoginCallbackInfo>) + Send + 'static,
    ) -> Result<()> {
        let token = CString::new(token)?;
        let display_name_cstr = match display_name {
            Some(v) => Some(CString::new(v)?),
            None => None,
        };

        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Connect_LoginCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_Connect_LoginCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }

        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });

        let credentials = sys::EOS_Connect_Credentials {
            ApiVersion: sys::EOS_CONNECT_CREDENTIALS_API_LATEST as i32,
            Token: token.as_ptr(),
            Type: sys::EOS_EExternalCredentialType_EOS_ECT_OPENID_ACCESS_TOKEN,
        };

        let user_login_info;
        let user_login_info_ptr = if let Some(name) = display_name_cstr.as_ref() {
            user_login_info = sys::EOS_Connect_UserLoginInfo {
                ApiVersion: sys::EOS_CONNECT_USERLOGININFO_API_LATEST as i32,
                DisplayName: name.as_ptr(),
                NsaIdToken: std::ptr::null(),
            };
            &user_login_info as *const sys::EOS_Connect_UserLoginInfo
        } else {
            std::ptr::null()
        };

        let options = sys::EOS_Connect_LoginOptions {
            ApiVersion: sys::EOS_CONNECT_LOGIN_API_LATEST as i32,
            Credentials: &credentials,
            UserLoginInfo: user_login_info_ptr,
        };

        unsafe {
            sys::EOS_Connect_Login(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
        Ok(())
    }

    pub fn create_user(
        &self,
        continuance_token: ContinuanceToken,
        cb: impl FnOnce(Result<sys::EOS_Connect_CreateUserCallbackInfo>) + Send + 'static,
    ) {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Connect_CreateUserCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_Connect_CreateUserCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }

        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });

        let options = sys::EOS_Connect_CreateUserOptions {
            ApiVersion: sys::EOS_CONNECT_CREATEUSER_API_LATEST as i32,
            ContinuanceToken: continuance_token.raw(),
        };

        unsafe {
            sys::EOS_Connect_CreateUser(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
    }

    pub fn logout(
        &self,
        local_user: ProductUserId,
        cb: impl FnOnce(Result<sys::EOS_Connect_LogoutCallbackInfo>) + Send + 'static,
    ) {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Connect_LogoutCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_Connect_LogoutCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }
        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });
        let options = sys::EOS_Connect_LogoutOptions {
            ApiVersion: sys::EOS_CONNECT_LOGOUT_API_LATEST as i32,
            LocalUserId: local_user.raw(),
        };
        unsafe {
            sys::EOS_Connect_Logout(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
    }

    pub fn link_account(
        &self,
        local_user: ProductUserId,
        continuance_token: ContinuanceToken,
        cb: impl FnOnce(Result<sys::EOS_Connect_LinkAccountCallbackInfo>) + Send + 'static,
    ) {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Connect_LinkAccountCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_Connect_LinkAccountCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }
        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });
        let options = sys::EOS_Connect_LinkAccountOptions {
            ApiVersion: sys::EOS_CONNECT_LINKACCOUNT_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            ContinuanceToken: continuance_token.raw(),
        };
        unsafe {
            sys::EOS_Connect_LinkAccount(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
    }

    pub fn unlink_account(
        &self,
        local_user: ProductUserId,
        cb: impl FnOnce(Result<sys::EOS_Connect_UnlinkAccountCallbackInfo>) + Send + 'static,
    ) {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Connect_UnlinkAccountCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_Connect_UnlinkAccountCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }
        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });
        let options = sys::EOS_Connect_UnlinkAccountOptions {
            ApiVersion: sys::EOS_CONNECT_UNLINKACCOUNT_API_LATEST as i32,
            LocalUserId: local_user.raw(),
        };
        unsafe {
            sys::EOS_Connect_UnlinkAccount(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
    }

    pub fn create_device_id(
        &self,
        device_model: &str,
        cb: impl FnOnce(Result<sys::EOS_Connect_CreateDeviceIdCallbackInfo>) + Send + 'static,
    ) -> Result<()> {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Connect_CreateDeviceIdCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_Connect_CreateDeviceIdCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }
        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });
        let model = CString::new(device_model)?;
        let options = sys::EOS_Connect_CreateDeviceIdOptions {
            ApiVersion: sys::EOS_CONNECT_CREATEDEVICEID_API_LATEST as i32,
            DeviceModel: model.as_ptr(),
        };
        unsafe {
            sys::EOS_Connect_CreateDeviceId(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
        Ok(())
    }

    pub fn delete_device_id(
        &self,
        cb: impl FnOnce(Result<sys::EOS_Connect_DeleteDeviceIdCallbackInfo>) + Send + 'static,
    ) {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Connect_DeleteDeviceIdCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_Connect_DeleteDeviceIdCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }
        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });
        let options = sys::EOS_Connect_DeleteDeviceIdOptions {
            ApiVersion: sys::EOS_CONNECT_DELETEDEVICEID_API_LATEST as i32,
        };
        unsafe {
            sys::EOS_Connect_DeleteDeviceId(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
    }

    pub fn transfer_device_id_account(
        &self,
        primary_local_user: ProductUserId,
        local_device_user: ProductUserId,
        product_user_to_preserve: ProductUserId,
        cb: impl FnOnce(Result<sys::EOS_Connect_TransferDeviceIdAccountCallbackInfo>) + Send + 'static,
    ) {
        #[repr(C)]
        struct Cb {
            f: Option<
                Box<dyn FnOnce(Result<sys::EOS_Connect_TransferDeviceIdAccountCallbackInfo>) + Send>,
            >,
        }
        unsafe extern "C" fn trampoline(
            data: *const sys::EOS_Connect_TransferDeviceIdAccountCallbackInfo,
        ) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }
        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });
        let options = sys::EOS_Connect_TransferDeviceIdAccountOptions {
            ApiVersion: sys::EOS_CONNECT_TRANSFERDEVICEIDACCOUNT_API_LATEST as i32,
            PrimaryLocalUserId: primary_local_user.raw(),
            LocalDeviceUserId: local_device_user.raw(),
            ProductUserIdToPreserve: product_user_to_preserve.raw(),
        };
        unsafe {
            sys::EOS_Connect_TransferDeviceIdAccount(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
    }
}

macro_rules! impl_raw_handle {
    ($ty:ident, $raw:ty) => {
        impl $ty {
            pub fn raw_handle(&self) -> $raw {
                self.0.as_ptr() as $raw
            }
        }
    };
}

impl_raw_handle!(Achievements, sys::EOS_HAchievements);
impl_raw_handle!(AntiCheatClient, sys::EOS_HAntiCheatClient);
impl_raw_handle!(AntiCheatServer, sys::EOS_HAntiCheatServer);
impl_raw_handle!(CustomInvites, sys::EOS_HCustomInvites);
impl_raw_handle!(Ecom, sys::EOS_HEcom);
impl_raw_handle!(Friends, sys::EOS_HFriends);
impl_raw_handle!(IntegratedPlatform, sys::EOS_HIntegratedPlatform);
impl_raw_handle!(Kws, sys::EOS_HKWS);
impl_raw_handle!(Leaderboards, sys::EOS_HLeaderboards);
impl_raw_handle!(Metrics, sys::EOS_HMetrics);
impl_raw_handle!(Mods, sys::EOS_HMods);
impl_raw_handle!(PlayerDataStorage, sys::EOS_HPlayerDataStorage);
impl_raw_handle!(Presence, sys::EOS_HPresence);
impl_raw_handle!(ProgressionSnapshot, sys::EOS_HProgressionSnapshot);
impl_raw_handle!(Reports, sys::EOS_HReports);
impl_raw_handle!(Rtc, sys::EOS_HRTC);
impl_raw_handle!(RtcAdmin, sys::EOS_HRTCAdmin);
impl_raw_handle!(Sanctions, sys::EOS_HSanctions);
impl_raw_handle!(Sessions, sys::EOS_HSessions);
impl_raw_handle!(Stats, sys::EOS_HStats);
impl_raw_handle!(TitleStorage, sys::EOS_HTitleStorage);
impl_raw_handle!(Ui, sys::EOS_HUI);
impl_raw_handle!(UserInfo, sys::EOS_HUserInfo);

impl Lobby {
    pub fn raw_handle(&self) -> sys::EOS_HLobby {
        self.0.as_ptr() as sys::EOS_HLobby
    }

    pub fn get_invite_count(&self, local_user: ProductUserId) -> u32 {
        let opts = sys::EOS_Lobby_GetInviteCountOptions {
            ApiVersion: sys::EOS_LOBBY_GETINVITECOUNT_API_LATEST as i32,
            LocalUserId: local_user.raw(),
        };
        unsafe { sys::EOS_Lobby_GetInviteCount(self.raw_handle(), &opts) }
    }

    pub fn get_invite_id_by_index(&self, local_user: ProductUserId, index: u32) -> Result<String> {
        let opts = sys::EOS_Lobby_GetInviteIdByIndexOptions {
            ApiVersion: sys::EOS_LOBBY_GETINVITEIDBYINDEX_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            Index: index,
        };
        let mut buf = vec![0i8; (sys::EOS_LOBBY_INVITEID_MAX_LENGTH + 1) as usize];
        let mut len = buf.len() as i32;
        let res =
            unsafe { sys::EOS_Lobby_GetInviteIdByIndex(self.raw_handle(), &opts, buf.as_mut_ptr(), &mut len) };
        ok(res)?;
        Ok(unsafe { CStr::from_ptr(buf.as_ptr()) }
            .to_string_lossy()
            .into_owned())
    }

    pub fn create_lobby_search(&self, max_results: u32) -> Result<LobbySearch> {
        let opts = sys::EOS_Lobby_CreateLobbySearchOptions {
            ApiVersion: sys::EOS_LOBBY_CREATELOBBYSEARCH_API_LATEST as i32,
            MaxResults: max_results,
        };
        let mut out: sys::EOS_HLobbySearch = std::ptr::null_mut();
        let res = unsafe { sys::EOS_Lobby_CreateLobbySearch(self.raw_handle(), &opts, &mut out) };
        ok(res)?;
        unsafe { LobbySearch::from_raw(out) }
    }

    pub fn copy_lobby_details_handle(
        &self,
        lobby_id: &str,
        local_user: ProductUserId,
    ) -> Result<LobbyDetails> {
        let lobby_id = CString::new(lobby_id)?;
        let opts = sys::EOS_Lobby_CopyLobbyDetailsHandleOptions {
            ApiVersion: sys::EOS_LOBBY_COPYLOBBYDETAILSHANDLE_API_LATEST as i32,
            LobbyId: lobby_id.as_ptr(),
            LocalUserId: local_user.raw(),
        };
        let mut out: sys::EOS_HLobbyDetails = std::ptr::null_mut();
        let res = unsafe { sys::EOS_Lobby_CopyLobbyDetailsHandle(self.raw_handle(), &opts, &mut out) };
        ok(res)?;
        unsafe { LobbyDetails::from_raw(out) }
    }

    pub fn update_lobby_modification(
        &self,
        local_user: ProductUserId,
        lobby_id: &str,
    ) -> Result<LobbyModification> {
        let lobby_id = CString::new(lobby_id)?;
        let opts = sys::EOS_Lobby_UpdateLobbyModificationOptions {
            ApiVersion: sys::EOS_LOBBY_UPDATELOBBYMODIFICATION_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            LobbyId: lobby_id.as_ptr(),
        };
        let mut out: sys::EOS_HLobbyModification = std::ptr::null_mut();
        let res = unsafe { sys::EOS_Lobby_UpdateLobbyModification(self.raw_handle(), &opts, &mut out) };
        ok(res)?;
        unsafe { LobbyModification::from_raw(out) }
    }

    pub fn get_rtc_room_name(&self, lobby_id: &str, local_user: ProductUserId) -> Result<String> {
        let lobby_id = CString::new(lobby_id)?;
        let opts = sys::EOS_Lobby_GetRTCRoomNameOptions {
            ApiVersion: sys::EOS_LOBBY_GETRTCROOMNAME_API_LATEST as i32,
            LobbyId: lobby_id.as_ptr(),
            LocalUserId: local_user.raw(),
        };
        // EOS headers do not expose a public max-length constant for RTC room name.
        // Use a conservative fixed buffer; EOS returns EOS_LimitExceeded if insufficient.
        let mut buf = vec![0i8; 256];
        let mut len = buf.len() as u32;
        let res =
            unsafe { sys::EOS_Lobby_GetRTCRoomName(self.raw_handle(), &opts, buf.as_mut_ptr(), &mut len) };
        ok(res)?;
        Ok(unsafe { CStr::from_ptr(buf.as_ptr()) }
            .to_string_lossy()
            .into_owned())
    }

    pub fn create_lobby(
        &self,
        local_user: ProductUserId,
        params: &CreateLobbyParams,
        cb: impl FnOnce(Result<sys::EOS_Lobby_CreateLobbyCallbackInfo>) + Send + 'static,
    ) -> Result<()> {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Lobby_CreateLobbyCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_Lobby_CreateLobbyCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }
        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });

        let bucket_id = CString::new(params.bucket_id.clone())?;
        let options = sys::EOS_Lobby_CreateLobbyOptions {
            ApiVersion: sys::EOS_LOBBY_CREATELOBBY_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            MaxLobbyMembers: params.max_lobby_members,
            PermissionLevel: params.permission_level,
            bPresenceEnabled: if params.presence_enabled { 1 } else { 0 },
            bAllowInvites: if params.allow_invites { 1 } else { 0 },
            BucketId: bucket_id.as_ptr(),
            bDisableHostMigration: if params.disable_host_migration { 1 } else { 0 },
            bEnableRTCRoom: if params.enable_rtc_room { 1 } else { 0 },
            LocalRTCOptions: std::ptr::null(),
            LobbyId: std::ptr::null(),
            bEnableJoinById: if params.enable_join_by_id { 1 } else { 0 },
            bRejoinAfterKickRequiresInvite: if params.rejoin_after_kick_requires_invite { 1 } else { 0 },
            AllowedPlatformIds: std::ptr::null(),
            AllowedPlatformIdsCount: 0,
            bCrossplayOptOut: 0,
            RTCRoomJoinActionType: sys::EOS_ELobbyRTCRoomJoinActionType_EOS_LRRJAT_AutomaticJoin,
        };

        unsafe {
            sys::EOS_Lobby_CreateLobby(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
        Ok(())
    }

    pub fn join_lobby(
        &self,
        lobby_details: &LobbyDetails,
        local_user: ProductUserId,
        presence_enabled: bool,
        cb: impl FnOnce(Result<sys::EOS_Lobby_JoinLobbyCallbackInfo>) + Send + 'static,
    ) {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Lobby_JoinLobbyCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_Lobby_JoinLobbyCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }
        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });
        let options = sys::EOS_Lobby_JoinLobbyOptions {
            ApiVersion: sys::EOS_LOBBY_JOINLOBBY_API_LATEST as i32,
            LobbyDetailsHandle: lobby_details.raw_handle(),
            LocalUserId: local_user.raw(),
            bPresenceEnabled: if presence_enabled { 1 } else { 0 },
            LocalRTCOptions: std::ptr::null(),
            bCrossplayOptOut: 0,
            RTCRoomJoinActionType: sys::EOS_ELobbyRTCRoomJoinActionType_EOS_LRRJAT_AutomaticJoin,
        };
        unsafe {
            sys::EOS_Lobby_JoinLobby(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
    }

    pub fn leave_lobby(
        &self,
        local_user: ProductUserId,
        lobby_id: &str,
        cb: impl FnOnce(Result<sys::EOS_Lobby_LeaveLobbyCallbackInfo>) + Send + 'static,
    ) -> Result<()> {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Lobby_LeaveLobbyCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_Lobby_LeaveLobbyCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }
        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });
        let lobby_id = CString::new(lobby_id)?;
        let options = sys::EOS_Lobby_LeaveLobbyOptions {
            ApiVersion: sys::EOS_LOBBY_LEAVELOBBY_API_LATEST as i32,
            LobbyId: lobby_id.as_ptr(),
            LocalUserId: local_user.raw(),
        };
        unsafe {
            sys::EOS_Lobby_LeaveLobby(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
        Ok(())
    }

    pub fn destroy_lobby(
        &self,
        local_user: ProductUserId,
        lobby_id: &str,
        cb: impl FnOnce(Result<sys::EOS_Lobby_DestroyLobbyCallbackInfo>) + Send + 'static,
    ) -> Result<()> {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_Lobby_DestroyLobbyCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_Lobby_DestroyLobbyCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }
        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });
        let lobby_id = CString::new(lobby_id)?;
        let options = sys::EOS_Lobby_DestroyLobbyOptions {
            ApiVersion: sys::EOS_LOBBY_DESTROYLOBBY_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            LobbyId: lobby_id.as_ptr(),
        };
        unsafe {
            sys::EOS_Lobby_DestroyLobby(
                self.raw_handle(),
                &options,
                cb_box.ptr as *mut _,
                Some(trampoline),
            );
        }
        Ok(())
    }
}

impl P2P {
    pub fn raw_handle(&self) -> sys::EOS_HP2P {
        self.0.as_ptr() as sys::EOS_HP2P
    }

    pub fn query_nat_type(
        &self,
        cb: impl FnOnce(Result<NatType>) + Send + 'static,
    ) {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<NatType>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_P2P_OnQueryNATTypeCompleteInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(NatType::from_raw((*data).NATType))
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }

        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });
        let opts = sys::EOS_P2P_QueryNATTypeOptions {
            ApiVersion: sys::EOS_P2P_QUERYNATTYPE_API_LATEST as i32,
        };
        unsafe {
            sys::EOS_P2P_QueryNATType(self.raw_handle(), &opts, cb_box.ptr as *mut _, Some(trampoline));
        }
    }

    pub fn get_nat_type(&self) -> Result<NatType> {
        let opts = sys::EOS_P2P_GetNATTypeOptions {
            ApiVersion: sys::EOS_P2P_GETNATTYPE_API_LATEST as i32,
        };
        let mut out = sys::EOS_ENATType_EOS_NAT_Unknown;
        let res = unsafe { sys::EOS_P2P_GetNATType(self.raw_handle(), &opts, &mut out) };
        ok(res)?;
        Ok(NatType::from_raw(out))
    }

    pub fn set_relay_control(&self, relay: RelayControl) -> Result<()> {
        let opts = sys::EOS_P2P_SetRelayControlOptions {
            ApiVersion: sys::EOS_P2P_SETRELAYCONTROL_API_LATEST as i32,
            RelayControl: relay.to_raw(),
        };
        let res = unsafe { sys::EOS_P2P_SetRelayControl(self.raw_handle(), &opts) };
        ok(res)
    }

    pub fn get_relay_control(&self) -> Result<RelayControl> {
        let opts = sys::EOS_P2P_GetRelayControlOptions {
            ApiVersion: sys::EOS_P2P_GETRELAYCONTROL_API_LATEST as i32,
        };
        let mut out = sys::EOS_ERelayControl_EOS_RC_AllowRelays;
        let res = unsafe { sys::EOS_P2P_GetRelayControl(self.raw_handle(), &opts, &mut out) };
        ok(res)?;
        Ok(RelayControl::from_raw(out))
    }

    pub fn set_port_range(&self, port: u16, max_additional_ports: u16) -> Result<()> {
        let opts = sys::EOS_P2P_SetPortRangeOptions {
            ApiVersion: sys::EOS_P2P_SETPORTRANGE_API_LATEST as i32,
            Port: port,
            MaxAdditionalPortsToTry: max_additional_ports,
        };
        let res = unsafe { sys::EOS_P2P_SetPortRange(self.raw_handle(), &opts) };
        ok(res)
    }

    pub fn get_port_range(&self) -> Result<(u16, u16)> {
        let opts = sys::EOS_P2P_GetPortRangeOptions {
            ApiVersion: sys::EOS_P2P_GETPORTRANGE_API_LATEST as i32,
        };
        let mut port = 0u16;
        let mut extra = 0u16;
        let res = unsafe { sys::EOS_P2P_GetPortRange(self.raw_handle(), &opts, &mut port, &mut extra) };
        ok(res)?;
        Ok((port, extra))
    }

    pub fn set_packet_queue_size(&self, incoming_max: u64, outgoing_max: u64) -> Result<()> {
        let opts = sys::EOS_P2P_SetPacketQueueSizeOptions {
            ApiVersion: sys::EOS_P2P_SETPACKETQUEUESIZE_API_LATEST as i32,
            IncomingPacketQueueMaxSizeBytes: incoming_max,
            OutgoingPacketQueueMaxSizeBytes: outgoing_max,
        };
        let res = unsafe { sys::EOS_P2P_SetPacketQueueSize(self.raw_handle(), &opts) };
        ok(res)
    }

    pub fn get_packet_queue_info(&self) -> Result<PacketQueueInfo> {
        let opts = sys::EOS_P2P_GetPacketQueueInfoOptions {
            ApiVersion: sys::EOS_P2P_GETPACKETQUEUEINFO_API_LATEST as i32,
        };
        let mut out = sys::EOS_P2P_PacketQueueInfo::default();
        let res = unsafe { sys::EOS_P2P_GetPacketQueueInfo(self.raw_handle(), &opts, &mut out) };
        ok(res)?;
        Ok(PacketQueueInfo {
            incoming_max_size_bytes: out.IncomingPacketQueueMaxSizeBytes,
            incoming_current_size_bytes: out.IncomingPacketQueueCurrentSizeBytes,
            incoming_current_packet_count: out.IncomingPacketQueueCurrentPacketCount,
            outgoing_max_size_bytes: out.OutgoingPacketQueueMaxSizeBytes,
            outgoing_current_size_bytes: out.OutgoingPacketQueueCurrentSizeBytes,
            outgoing_current_packet_count: out.OutgoingPacketQueueCurrentPacketCount,
        })
    }

    pub fn send_packet(
        &self,
        local_user: ProductUserId,
        remote_user: ProductUserId,
        socket_name: &str,
        channel: u8,
        data: &[u8],
        reliability: PacketReliability,
        allow_delayed_delivery: bool,
        disable_auto_accept_connection: bool,
    ) -> Result<()> {
        let socket = make_socket_id(socket_name)?;
        let opts = sys::EOS_P2P_SendPacketOptions {
            ApiVersion: sys::EOS_P2P_SENDPACKET_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            RemoteUserId: remote_user.raw(),
            SocketId: &socket,
            Channel: channel,
            DataLengthBytes: data.len() as u32,
            Data: data.as_ptr() as *const _,
            bAllowDelayedDelivery: if allow_delayed_delivery { 1 } else { 0 },
            Reliability: reliability.to_raw(),
            bDisableAutoAcceptConnection: if disable_auto_accept_connection { 1 } else { 0 },
        };
        let res = unsafe { sys::EOS_P2P_SendPacket(self.raw_handle(), &opts) };
        ok(res)
    }

    pub fn get_next_received_packet_size(
        &self,
        local_user: ProductUserId,
        requested_channel: Option<u8>,
    ) -> Result<u32> {
        let requested = requested_channel.unwrap_or_default();
        let requested_ptr = if requested_channel.is_some() {
            &requested as *const u8
        } else {
            std::ptr::null()
        };
        let opts = sys::EOS_P2P_GetNextReceivedPacketSizeOptions {
            ApiVersion: sys::EOS_P2P_GETNEXTRECEIVEDPACKETSIZE_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            RequestedChannel: requested_ptr,
        };
        let mut size = 0u32;
        let res = unsafe { sys::EOS_P2P_GetNextReceivedPacketSize(self.raw_handle(), &opts, &mut size) };
        ok(res)?;
        Ok(size)
    }

    pub fn receive_packet(
        &self,
        local_user: ProductUserId,
        max_data_size_bytes: u32,
        requested_channel: Option<u8>,
    ) -> Result<ReceivedPacket> {
        let requested = requested_channel.unwrap_or_default();
        let requested_ptr = if requested_channel.is_some() {
            &requested as *const u8
        } else {
            std::ptr::null()
        };
        let opts = sys::EOS_P2P_ReceivePacketOptions {
            ApiVersion: sys::EOS_P2P_RECEIVEPACKET_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            MaxDataSizeBytes: max_data_size_bytes,
            RequestedChannel: requested_ptr,
        };
        let mut peer = std::ptr::null_mut();
        let mut socket = sys::EOS_P2P_SocketId::default();
        let mut channel = 0u8;
        let mut data = vec![0u8; max_data_size_bytes as usize];
        let mut bytes_written = 0u32;
        let res = unsafe {
            sys::EOS_P2P_ReceivePacket(
                self.raw_handle(),
                &opts,
                &mut peer,
                &mut socket,
                &mut channel,
                data.as_mut_ptr() as *mut _,
                &mut bytes_written,
            )
        };
        ok(res)?;
        data.truncate(bytes_written as usize);
        Ok(ReceivedPacket {
            peer_id: ProductUserId(peer),
            socket_name: socket_name_from_raw(&socket),
            channel,
            data,
        })
    }

    pub fn accept_connection(
        &self,
        local_user: ProductUserId,
        remote_user: ProductUserId,
        socket_name: &str,
    ) -> Result<()> {
        let socket = make_socket_id(socket_name)?;
        let opts = sys::EOS_P2P_AcceptConnectionOptions {
            ApiVersion: sys::EOS_P2P_ACCEPTCONNECTION_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            RemoteUserId: remote_user.raw(),
            SocketId: &socket,
        };
        ok(unsafe { sys::EOS_P2P_AcceptConnection(self.raw_handle(), &opts) })
    }

    pub fn close_connection(
        &self,
        local_user: ProductUserId,
        remote_user: ProductUserId,
        socket_name: Option<&str>,
    ) -> Result<()> {
        let socket = match socket_name {
            Some(n) => Some(make_socket_id(n)?),
            None => None,
        };
        let opts = sys::EOS_P2P_CloseConnectionOptions {
            ApiVersion: sys::EOS_P2P_CLOSECONNECTION_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            RemoteUserId: remote_user.raw(),
            SocketId: socket
                .as_ref()
                .map(|s| s as *const _)
                .unwrap_or(std::ptr::null()),
        };
        ok(unsafe { sys::EOS_P2P_CloseConnection(self.raw_handle(), &opts) })
    }

    pub fn close_connections(&self, local_user: ProductUserId, socket_name: &str) -> Result<()> {
        let socket = make_socket_id(socket_name)?;
        let opts = sys::EOS_P2P_CloseConnectionsOptions {
            ApiVersion: sys::EOS_P2P_CLOSECONNECTIONS_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            SocketId: &socket,
        };
        ok(unsafe { sys::EOS_P2P_CloseConnections(self.raw_handle(), &opts) })
    }

    pub fn clear_packet_queue(
        &self,
        local_user: ProductUserId,
        remote_user: ProductUserId,
        socket_name: &str,
    ) -> Result<()> {
        let socket = make_socket_id(socket_name)?;
        let opts = sys::EOS_P2P_ClearPacketQueueOptions {
            ApiVersion: sys::EOS_P2P_CLEARPACKETQUEUE_API_LATEST as i32,
            LocalUserId: local_user.raw(),
            RemoteUserId: remote_user.raw(),
            SocketId: &socket,
        };
        ok(unsafe { sys::EOS_P2P_ClearPacketQueue(self.raw_handle(), &opts) })
    }
}

// ---- Owned EOS objects with explicit Release() ----

macro_rules! owned_ptr_release {
    ($name:ident, $inner:ty, $release:path) => {
        pub struct $name(NonNull<$inner>);

        impl $name {
            pub unsafe fn from_raw(ptr: *mut $inner) -> Result<Self> {
                Ok(Self(NonNull::new(ptr).ok_or(Error::Null)?))
            }

            pub fn as_ptr(&self) -> *mut $inner {
                self.0.as_ptr()
            }

            pub fn into_raw(self) -> *mut $inner {
                let p = self.0.as_ptr();
                std::mem::forget(self);
                p
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                unsafe { $release(self.0.as_ptr()) }
            }
        }
    };
}

macro_rules! owned_handle_release {
    ($name:ident, $handle:ty, $release:path) => {
        pub struct $name($handle);

        impl $name {
            pub unsafe fn from_raw(h: $handle) -> Result<Self> {
                if (h as usize) == 0 {
                    return Err(Error::Null);
                }
                Ok(Self(h))
            }

            pub fn raw_handle(&self) -> $handle {
                self.0
            }

            pub fn into_raw(self) -> $handle {
                let h = self.0;
                std::mem::forget(self);
                h
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                unsafe { $release(self.0) }
            }
        }
    };
}

owned_ptr_release!(AuthToken, sys::EOS_Auth_Token, sys::EOS_Auth_Token_Release);
owned_ptr_release!(AuthIdToken, sys::EOS_Auth_IdToken, sys::EOS_Auth_IdToken_Release);
owned_ptr_release!(
    ConnectExternalAccountInfo,
    sys::EOS_Connect_ExternalAccountInfo,
    sys::EOS_Connect_ExternalAccountInfo_Release
);
owned_ptr_release!(ConnectIdToken, sys::EOS_Connect_IdToken, sys::EOS_Connect_IdToken_Release);
owned_ptr_release!(
    EcomEntitlement,
    sys::EOS_Ecom_Entitlement,
    sys::EOS_Ecom_Entitlement_Release
);
owned_ptr_release!(
    EcomCatalogItem,
    sys::EOS_Ecom_CatalogItem,
    sys::EOS_Ecom_CatalogItem_Release
);
owned_ptr_release!(
    EcomCatalogOffer,
    sys::EOS_Ecom_CatalogOffer,
    sys::EOS_Ecom_CatalogOffer_Release
);
owned_ptr_release!(
    EcomKeyImageInfo,
    sys::EOS_Ecom_KeyImageInfo,
    sys::EOS_Ecom_KeyImageInfo_Release
);
owned_ptr_release!(
    EcomCatalogRelease,
    sys::EOS_Ecom_CatalogRelease,
    sys::EOS_Ecom_CatalogRelease_Release
);
owned_handle_release!(
    EcomTransaction,
    sys::EOS_Ecom_HTransaction,
    sys::EOS_Ecom_Transaction_Release
);
owned_ptr_release!(PresenceInfo, sys::EOS_Presence_Info, sys::EOS_Presence_Info_Release);
owned_handle_release!(
    PresenceModification,
    sys::EOS_HPresenceModification,
    sys::EOS_PresenceModification_Release
);
owned_handle_release!(
    SessionModification,
    sys::EOS_HSessionModification,
    sys::EOS_SessionModification_Release
);
owned_handle_release!(
    ActiveSession,
    sys::EOS_HActiveSession,
    sys::EOS_ActiveSession_Release
);
owned_handle_release!(
    SessionDetails,
    sys::EOS_HSessionDetails,
    sys::EOS_SessionDetails_Release
);
owned_handle_release!(
    SessionSearch,
    sys::EOS_HSessionSearch,
    sys::EOS_SessionSearch_Release
);
owned_ptr_release!(
    SessionDetailsAttribute,
    sys::EOS_SessionDetails_Attribute,
    sys::EOS_SessionDetails_Attribute_Release
);
owned_ptr_release!(
    SessionDetailsInfo,
    sys::EOS_SessionDetails_Info,
    sys::EOS_SessionDetails_Info_Release
);
owned_ptr_release!(
    ActiveSessionInfo,
    sys::EOS_ActiveSession_Info,
    sys::EOS_ActiveSession_Info_Release
);
owned_handle_release!(
    LobbyModification,
    sys::EOS_HLobbyModification,
    sys::EOS_LobbyModification_Release
);
owned_handle_release!(LobbyDetails, sys::EOS_HLobbyDetails, sys::EOS_LobbyDetails_Release);
owned_handle_release!(LobbySearch, sys::EOS_HLobbySearch, sys::EOS_LobbySearch_Release);
owned_ptr_release!(
    LobbyDetailsInfo,
    sys::EOS_LobbyDetails_Info,
    sys::EOS_LobbyDetails_Info_Release
);
owned_ptr_release!(
    LobbyAttribute,
    sys::EOS_Lobby_Attribute,
    sys::EOS_Lobby_Attribute_Release
);
owned_ptr_release!(
    LobbyMemberInfo,
    sys::EOS_LobbyDetails_MemberInfo,
    sys::EOS_LobbyDetails_MemberInfo_Release
);
owned_ptr_release!(UserInfoData, sys::EOS_UserInfo, sys::EOS_UserInfo_Release);
owned_ptr_release!(
    ExternalUserInfo,
    sys::EOS_UserInfo_ExternalUserInfo,
    sys::EOS_UserInfo_ExternalUserInfo_Release
);
owned_ptr_release!(
    BestDisplayName,
    sys::EOS_UserInfo_BestDisplayName,
    sys::EOS_UserInfo_BestDisplayName_Release
);
owned_ptr_release!(
    PlayerDataStorageFileMetadata,
    sys::EOS_PlayerDataStorage_FileMetadata,
    sys::EOS_PlayerDataStorage_FileMetadata_Release
);
owned_handle_release!(
    PlayerDataStorageFileTransferRequest,
    sys::EOS_HPlayerDataStorageFileTransferRequest,
    sys::EOS_PlayerDataStorageFileTransferRequest_Release
);
owned_ptr_release!(
    TitleStorageFileMetadata,
    sys::EOS_TitleStorage_FileMetadata,
    sys::EOS_TitleStorage_FileMetadata_Release
);
owned_handle_release!(
    TitleStorageFileTransferRequest,
    sys::EOS_HTitleStorageFileTransferRequest,
    sys::EOS_TitleStorageFileTransferRequest_Release
);
owned_ptr_release!(
    AchievementsDefinitionV2,
    sys::EOS_Achievements_DefinitionV2,
    sys::EOS_Achievements_DefinitionV2_Release
);
owned_ptr_release!(
    AchievementsPlayerAchievement,
    sys::EOS_Achievements_PlayerAchievement,
    sys::EOS_Achievements_PlayerAchievement_Release
);
owned_ptr_release!(
    AchievementsDefinition,
    sys::EOS_Achievements_Definition,
    sys::EOS_Achievements_Definition_Release
);
owned_ptr_release!(
    AchievementsUnlockedAchievement,
    sys::EOS_Achievements_UnlockedAchievement,
    sys::EOS_Achievements_UnlockedAchievement_Release
);
owned_ptr_release!(StatsStat, sys::EOS_Stats_Stat, sys::EOS_Stats_Stat_Release);
owned_ptr_release!(
    LeaderboardsDefinition,
    sys::EOS_Leaderboards_Definition,
    sys::EOS_Leaderboards_Definition_Release
);
owned_ptr_release!(
    LeaderboardsUserScore,
    sys::EOS_Leaderboards_LeaderboardUserScore,
    sys::EOS_Leaderboards_LeaderboardUserScore_Release
);
owned_ptr_release!(
    LeaderboardsRecord,
    sys::EOS_Leaderboards_LeaderboardRecord,
    sys::EOS_Leaderboards_LeaderboardRecord_Release
);
owned_ptr_release!(
    LeaderboardsLeaderboardDefinition,
    sys::EOS_Leaderboards_Definition,
    sys::EOS_Leaderboards_LeaderboardDefinition_Release
);
owned_ptr_release!(ModsModInfo, sys::EOS_Mods_ModInfo, sys::EOS_Mods_ModInfo_Release);
owned_ptr_release!(
    SanctionsPlayerSanction,
    sys::EOS_Sanctions_PlayerSanction,
    sys::EOS_Sanctions_PlayerSanction_Release
);
owned_ptr_release!(
    KwsPermissionStatus,
    sys::EOS_KWS_PermissionStatus,
    sys::EOS_KWS_PermissionStatus_Release
);
owned_ptr_release!(
    RtcAdminUserToken,
    sys::EOS_RTCAdmin_UserToken,
    sys::EOS_RTCAdmin_UserToken_Release
);

impl LobbySearch {
    pub fn set_lobby_id(&self, lobby_id: &str) -> Result<()> {
        let lobby_id = CString::new(lobby_id)?;
        let opts = sys::EOS_LobbySearch_SetLobbyIdOptions {
            ApiVersion: sys::EOS_LOBBYSEARCH_SETLOBBYID_API_LATEST as i32,
            LobbyId: lobby_id.as_ptr(),
        };
        ok(unsafe { sys::EOS_LobbySearch_SetLobbyId(self.raw_handle(), &opts) })
    }

    pub fn set_target_user_id(&self, target_user_id: ProductUserId) -> Result<()> {
        let opts = sys::EOS_LobbySearch_SetTargetUserIdOptions {
            ApiVersion: sys::EOS_LOBBYSEARCH_SETTARGETUSERID_API_LATEST as i32,
            TargetUserId: target_user_id.raw(),
        };
        ok(unsafe { sys::EOS_LobbySearch_SetTargetUserId(self.raw_handle(), &opts) })
    }

    pub fn set_max_results(&self, max_results: u32) -> Result<()> {
        let opts = sys::EOS_LobbySearch_SetMaxResultsOptions {
            ApiVersion: sys::EOS_LOBBYSEARCH_SETMAXRESULTS_API_LATEST as i32,
            MaxResults: max_results,
        };
        ok(unsafe { sys::EOS_LobbySearch_SetMaxResults(self.raw_handle(), &opts) })
    }

    pub fn set_parameter(
        &self,
        key: &str,
        value: &LobbySearchValue,
        comparison_op: sys::EOS_EComparisonOp,
    ) -> Result<()> {
        let key = CString::new(key)?;
        let string_storage;
        let (value_union, value_type) = match value {
            LobbySearchValue::Bool(v) => (
                sys::_tagEOS_Lobby_AttributeData__bindgen_ty_1 {
                    AsBool: if *v { 1 } else { 0 },
                },
                sys::EOS_EAttributeType_EOS_AT_BOOLEAN,
            ),
            LobbySearchValue::Int64(v) => (
                sys::_tagEOS_Lobby_AttributeData__bindgen_ty_1 { AsInt64: *v },
                sys::EOS_EAttributeType_EOS_AT_INT64,
            ),
            LobbySearchValue::Double(v) => (
                sys::_tagEOS_Lobby_AttributeData__bindgen_ty_1 { AsDouble: *v },
                sys::EOS_EAttributeType_EOS_AT_DOUBLE,
            ),
            LobbySearchValue::String(v) => {
                string_storage = CString::new(v.as_str())?;
                (
                    sys::_tagEOS_Lobby_AttributeData__bindgen_ty_1 {
                        AsUtf8: string_storage.as_ptr(),
                    },
                    sys::EOS_EAttributeType_EOS_AT_STRING,
                )
            }
        };

        let attr = sys::EOS_Lobby_AttributeData {
            ApiVersion: sys::EOS_LOBBY_ATTRIBUTEDATA_API_LATEST as i32,
            Key: key.as_ptr(),
            Value: value_union,
            ValueType: value_type,
        };

        let opts = sys::EOS_LobbySearch_SetParameterOptions {
            ApiVersion: sys::EOS_LOBBYSEARCH_SETPARAMETER_API_LATEST as i32,
            Parameter: &attr,
            ComparisonOp: comparison_op,
        };
        ok(unsafe { sys::EOS_LobbySearch_SetParameter(self.raw_handle(), &opts) })
    }

    pub fn remove_parameter(&self, key: &str, comparison_op: sys::EOS_EComparisonOp) -> Result<()> {
        let key = CString::new(key)?;
        let opts = sys::EOS_LobbySearch_RemoveParameterOptions {
            ApiVersion: sys::EOS_LOBBYSEARCH_REMOVEPARAMETER_API_LATEST as i32,
            Key: key.as_ptr(),
            ComparisonOp: comparison_op,
        };
        ok(unsafe { sys::EOS_LobbySearch_RemoveParameter(self.raw_handle(), &opts) })
    }

    pub fn find(
        &self,
        local_user: ProductUserId,
        cb: impl FnOnce(Result<sys::EOS_LobbySearch_FindCallbackInfo>) + Send + 'static,
    ) {
        #[repr(C)]
        struct Cb {
            f: Option<Box<dyn FnOnce(Result<sys::EOS_LobbySearch_FindCallbackInfo>) + Send>>,
        }
        unsafe extern "C" fn trampoline(data: *const sys::EOS_LobbySearch_FindCallbackInfo) {
            let client_data = (*data).ClientData as *mut Cb;
            let mut boxed = Box::from_raw(client_data);
            let res = if (*data).ResultCode == sys::EOS_EResult_EOS_Success {
                Ok(*data)
            } else {
                Err(Error::Eos((*data).ResultCode))
            };
            if let Some(f) = boxed.f.take() {
                f(res);
            }
        }

        let cb_box = CallbackOnce::new(Cb {
            f: Some(Box::new(cb)),
        });
        let opts = sys::EOS_LobbySearch_FindOptions {
            ApiVersion: sys::EOS_LOBBYSEARCH_FIND_API_LATEST as i32,
            LocalUserId: local_user.raw(),
        };
        unsafe {
            sys::EOS_LobbySearch_Find(self.raw_handle(), &opts, cb_box.ptr as *mut _, Some(trampoline));
        }
    }

    pub fn get_search_result_count(&self) -> u32 {
        let opts = sys::EOS_LobbySearch_GetSearchResultCountOptions {
            ApiVersion: sys::EOS_LOBBYSEARCH_GETSEARCHRESULTCOUNT_API_LATEST as i32,
        };
        unsafe { sys::EOS_LobbySearch_GetSearchResultCount(self.raw_handle(), &opts) }
    }

    pub fn copy_search_result_by_index(&self, lobby_index: u32) -> Result<LobbyDetails> {
        let opts = sys::EOS_LobbySearch_CopySearchResultByIndexOptions {
            ApiVersion: sys::EOS_LOBBYSEARCH_COPYSEARCHRESULTBYINDEX_API_LATEST as i32,
            LobbyIndex: lobby_index,
        };
        let mut out: sys::EOS_HLobbyDetails = std::ptr::null_mut();
        let res = unsafe { sys::EOS_LobbySearch_CopySearchResultByIndex(self.raw_handle(), &opts, &mut out) };
        ok(res)?;
        unsafe { LobbyDetails::from_raw(out) }
    }
}

impl AuthToken {
    pub fn account_id(&self) -> EpicAccountId {
        // SAFETY: pointer is owned by this RAII wrapper and valid for its lifetime.
        let token = unsafe { &*self.as_ptr() };
        EpicAccountId(token.AccountId)
    }

    pub fn access_token(&self) -> Option<&str> {
        let token = unsafe { &*self.as_ptr() };
        if token.AccessToken.is_null() {
            return None;
        }
        Some(unsafe { CStr::from_ptr(token.AccessToken) }.to_str().ok()?)
    }
}

impl AuthIdToken {
    pub fn account_id(&self) -> EpicAccountId {
        let token = unsafe { &*self.as_ptr() };
        EpicAccountId(token.AccountId)
    }

    pub fn json_web_token(&self) -> Option<&str> {
        let token = unsafe { &*self.as_ptr() };
        if token.JsonWebToken.is_null() {
            return None;
        }
        Some(unsafe { CStr::from_ptr(token.JsonWebToken) }.to_str().ok()?)
    }
}

impl ConnectIdToken {
    pub fn product_user_id(&self) -> ProductUserId {
        let token = unsafe { &*self.as_ptr() };
        ProductUserId(token.ProductUserId)
    }

    pub fn json_web_token(&self) -> Option<&str> {
        let token = unsafe { &*self.as_ptr() };
        if token.JsonWebToken.is_null() {
            return None;
        }
        Some(unsafe { CStr::from_ptr(token.JsonWebToken) }.to_str().ok()?)
    }
}


