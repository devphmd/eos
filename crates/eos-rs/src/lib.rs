use std::ffi::CString;
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
}

impl P2P {
    pub fn raw_handle(&self) -> sys::EOS_HP2P {
        self.0.as_ptr() as sys::EOS_HP2P
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


