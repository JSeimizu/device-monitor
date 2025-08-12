use azure_core::date::OffsetDateTime;
use azure_storage::prelude::BlobSasPermissions;
#[allow(unused)]
use {
    super::error::DMError,
    super::mqtt_ctrl::evp::evp_state::UUID,
    super::mqtt_ctrl::evp::module::ModuleInfo,
    azure_storage::{CloudLocation, StorageCredentials},
    azure_storage_blobs::{
        container::operations::list_blobs::BlobItem, prelude::*,
        service::operations::ListContainersResponse,
    },
    bytes::Bytes,
    clap::Parser,
    error_stack::{Context, Report, Result, ResultExt},
    futures::stream::{self, StreamExt},
    jlogger_tracing::{JloggerBuilder, LevelFilter, jdebug, jerror, jinfo},
    sha2::{Digest, Sha256},
    std::{
        collections::HashMap,
        io::Read,
        sync::{Mutex, OnceLock},
    },
};

const ACCOUNT_NAME: &str = "devstoreaccount1";
const ACCOUNT_KEY: &str =
    "Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==";

/// Global AzuriteStorage instance protected by mutex for thread safety
static GLOBAL_AZURITE_STORAGE: OnceLock<Mutex<Option<AzuriteStorage>>> = OnceLock::new();

/// Global AzuriteStorage configuration URL for retry attempts
static GLOBAL_AZURITE_URL: OnceLock<String> = OnceLock::new();

/// Initialize the global AzuriteStorage instance
pub fn init_global_azurite_storage(azurite_url: &str) -> Result<(), DMError> {
    // Store the URL for retry attempts
    GLOBAL_AZURITE_URL
        .set(azurite_url.to_string())
        .map_err(|_| DMError::InvalidData)?;

    let storage = AzuriteStorage::new(azurite_url).ok();
    GLOBAL_AZURITE_STORAGE
        .set(Mutex::new(storage))
        .map_err(|_| DMError::InvalidData)?;

    // If storage was successfully created, scan for existing token providers
    with_azurite_storage_mut(|storage| {
        let _ = storage.scan_upload_containers();
    });

    Ok(())
}

/// Get reference to global AzuriteStorage mutex (for internal use)
fn get_global_azurite_storage_ref() -> &'static Mutex<Option<AzuriteStorage>> {
    GLOBAL_AZURITE_STORAGE
        .get()
        .expect("Global AzuriteStorage not initialized - call init_global_azurite_storage first")
}

/// Access global AzuriteStorage with closure for immutable operations
pub fn with_azurite_storage<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&AzuriteStorage) -> R,
{
    let storage_guard = get_global_azurite_storage_ref()
        .lock()
        .expect("Failed to lock global AzuriteStorage mutex");

    if let Some(ref storage) = *storage_guard {
        Some(f(storage))
    } else {
        None
    }
}

/// Access global AzuriteStorage with closure for mutable operations
pub fn with_azurite_storage_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AzuriteStorage) -> R,
{
    let mut storage_guard = get_global_azurite_storage_ref()
        .lock()
        .expect("Failed to lock global AzuriteStorage mutex");

    if let Some(ref mut storage) = *storage_guard {
        Some(f(storage))
    } else {
        None
    }
}

/// Try to reinitialize AzuriteStorage if it's currently None
pub fn try_reinit_azurite_storage() -> bool {
    let azurite_url = GLOBAL_AZURITE_URL
        .get()
        .expect("Global AzuriteStorage URL not initialized");

    let mut storage_guard = get_global_azurite_storage_ref()
        .lock()
        .expect("Failed to lock global AzuriteStorage mutex");

    if storage_guard.is_none() {
        if let Ok(new_storage) = AzuriteStorage::new(azurite_url) {
            *storage_guard = Some(new_storage);
            // Scan for existing token providers after successful initialization
            if let Some(ref mut storage) = *storage_guard {
                let _ = storage.scan_upload_containers();
            }
            jinfo!("AzuriteStorage reinitialized successfully");
            return true;
        } else {
            jdebug!("Failed to reinitialize AzuriteStorage");
        }
    }
    false
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AzuriteAction {
    Add,

    #[default]
    Deploy,
}

#[derive(Debug, Clone)]
pub struct TokenProvider {
    pub uuid: UUID,
    pub container: String,
}

#[derive(Debug, Clone)]
pub struct UiBlob {
    pub name: String,
    pub created_on: chrono::DateTime<chrono::Utc>,
    pub size: u64,
}

pub struct AzuriteStorage {
    runtime: tokio::runtime::Runtime,
    blob_service_client: BlobServiceClient,
    module_info_db: HashMap<UUID, ModuleInfo>,
    current_module_id: usize,
    new_module: String,
    action: AzuriteAction,
    token_providers: HashMap<UUID, TokenProvider>,
    current_token_provider_id: usize,
}

#[allow(unused)]
impl AzuriteStorage {
    pub fn new(azurite_url: &str) -> Result<Self, DMError> {
        let runtime = tokio::runtime::Runtime::new().map_err(|e| {
            Report::new(DMError::RuntimeError)
                .attach_printable("Failed to create Tokio runtime")
                .attach(e)
        })?;

        let credential = StorageCredentials::access_key(ACCOUNT_NAME, ACCOUNT_KEY);

        let (address, port) = azurite_url
            .trim_end_matches('/')
            .trim_start_matches("https://")
            .split_once(':')
            .map(|(address, port)| {
                let port: u16 = port
                    .parse()
                    .unwrap_or_else(|_| panic!("invalid port: {}", port));
                (address.to_owned(), port)
            })
            .ok_or_else(|| {
                Report::new(DMError::InvalidData)
                    .attach_printable(format!("Invalid URL: {azurite_url}"))
            })?;
        let client_builder =
            ClientBuilder::with_location(CloudLocation::Emulator { address, port }, credential);

        let azure_storage = AzuriteStorage {
            runtime,
            blob_service_client: client_builder.blob_service_client(),
            module_info_db: HashMap::new(),
            current_module_id: 0,
            action: AzuriteAction::default(),
            new_module: String::new(),
            token_providers: HashMap::new(),
            current_token_provider_id: 0,
        };

        Ok(azure_storage)
    }

    pub fn is_container_exists(&self, container_name: &str) -> bool {
        let container_client = self.blob_service_client.container_client(container_name);
        self.runtime.block_on(async {
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                    jerror!("Timeout while checking if container exists, returning false");
                    false
                }

                exists = container_client.exists() => {
                    match exists {
                        Ok(exists) => exists,
                        Err(e) => {
                            jerror!(
                                "Failed to check if container '{}' exists: {}",
                                container_name,
                                e
                            );
                            false
                        }
                    }
                }


            }
        })
    }

    pub fn list_containers(&self) -> Vec<String> {
        let mut result = Vec::new();
        self.runtime.block_on(async {
            let mut stream = self.blob_service_client.list_containers().into_stream();

            loop {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                        jerror!("Timeout while listing containers, returning empty list");
                        break;
                    }

                next = stream.next() => {
                        if let Some(Ok(response)) = next {
                            let ListContainersResponse {
                                containers,
                                next_marker: _,
                            } = response;

                            for container in containers {
                                result.push(container.name.clone());
                            }
                        } else {
                            break;
                        }
                    }
                }
            }

            result
        })
    }

    pub fn create_container(&self, container_name: &str) -> Result<(), DMError> {
        self.runtime.block_on(async {
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                    jerror!("Timeout while creating container, returning error");
                    Err(Report::new(DMError::Timeout))
                }

                response = self.blob_service_client.container_client(container_name).create() => {
                    response.map_err(|e| {
                        Report::new(DMError::IOError).attach_printable(format!(
                            "Failed to create container '{}': {}",
                            container_name, e
                        ))
                    })
                }
            }
        })
    }

    pub fn delete_container(&self, container_name: &str) -> Result<(), DMError> {
        self.runtime.block_on(async {
            self.blob_service_client
                .container_client(container_name)
                .delete()
                .await
                .map_err(|e| {
                    Report::new(DMError::IOError).attach_printable(format!(
                        "Failed to delete container '{}': {}",
                        container_name, e
                    ))
                })
        })
    }

    pub fn container_url(&self, container_name: &str) -> Result<String, DMError> {
        self.runtime.block_on(async {
            Ok(self
                .blob_service_client
                .container_client(container_name)
                .url()
                .map_err(|e| {
                    Report::new(DMError::InvalidData).attach_printable(format!(
                        "Failed to get URL for container '{}': {}",
                        container_name, e
                    ))
                })?
                .path()
                .to_owned())
        })
    }

    pub fn create_container_if_not_exists(&self, container_name: &str) -> Result<(), DMError> {
        if !self.is_container_exists(container_name) {
            self.create_container(container_name).map_err(|e| {
                Report::new(DMError::IOError)
                    .attach_printable(format!("Failed to create container '{}'", container_name))
                    .attach(e)
            })
        } else {
            Ok(())
        }
    }

    pub fn push_blob(
        &mut self,
        container_name: Option<&str>,
        file_path: &str,
    ) -> Result<(), DMError> {
        let file = std::fs::File::open(file_path).map_err(|e| {
            Report::new(DMError::IOError).attach_printable(format!("Failed to open file: {}", e))
        })?;
        let mut reader = std::io::BufReader::new(file);
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).map_err(|e| {
            Report::new(DMError::IOError).attach_printable(format!("Failed to read file: {}", e))
        })?;

        let mut hasher = Sha256::new();
        hasher.update(&buf);
        let hash = format!("{:x}", hasher.finalize());
        let container_name = container_name.unwrap_or("default");

        self.create_container_if_not_exists(container_name)
            .map_err(|e| {
                Report::new(DMError::IOError)
                    .attach_printable(format!("Failed to create container '{}'", container_name))
                    .attach(e)
            })?;

        if let Some(file_name) = std::path::Path::new(file_path)
            .file_name()
            .and_then(|s| s.to_str())
        {
            let blob_client = self
                .blob_service_client
                .container_client(container_name)
                .blob_client(file_name);

            self.runtime.block_on(async {
                tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                            jerror!("Timeout while uploading blob, returning error");
                            Err(Report::new(DMError::Timeout))
                        }

                        response = blob_client.put_block_blob(Bytes::from(buf.clone())) => {
                            response.map_err(|e| {
                                Report::new(DMError::IOError).attach_printable(format!(
                                    "Failed to upload file to container '{}': {}",
                                    container_name, e
                                ))
                            })
                        }
                }
            })?;

            let module_info = ModuleInfo {
                id: UUID::new(),
                blob_name: file_path.to_string(),
                container_name: container_name.to_string(),
                hash,
                sas_url: String::new(), // Will be set later if needed
            };

            self.module_info_db
                .insert(module_info.id.clone(), module_info);

            Ok(())
        } else {
            Err(Report::new(DMError::InvalidData)
                .attach_printable("Failed to extract file name from the provided path"))
        }
    }

    pub fn get_blob(&self, container_name: Option<&str>, blob: &str) -> Result<Vec<u8>, DMError> {
        let blob_client = self
            .blob_service_client
            .container_client(container_name.unwrap_or("default"))
            .blob_client(blob);

        self.runtime.block_on(async {
            tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                        jerror!("Timeout while downloading blob, returning error");
                        Err(Report::new(DMError::Timeout))
                    }

                    response = blob_client.get_content() => {
                        response.map_err(|e| {
                            Report::new(DMError::IOError).attach_printable(format!(
                                "Failed to download file from container '{}': {}",
                                container_name.unwrap_or("default"), e
                            ))
                        })
                    }
            }
        })
    }

    pub fn remove_blob(&self, container_name: Option<&str>, blob: &str) -> Result<(), DMError> {
        let blob_client = self
            .blob_service_client
            .container_client(container_name.unwrap_or("default"))
            .blob_client(blob);

        self.runtime.block_on(async {
            tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                        jerror!("Timeout while uploading blob, returning error");
                        Err(Report::new(DMError::Timeout))
                    }

                    response = blob_client.delete() => {
                        response.map_err(|e| {
                            Report::new(DMError::IOError).attach_printable(format!(
                                "Failed to delete file from container '{}': {}",
                                container_name.unwrap_or("default"), e
                            ))
                        })
                    }
            }
        })?;

        Ok(())
    }

    pub fn list_blobs(&self, container_name: &str) -> Result<Vec<Blob>, DMError> {
        self.create_container_if_not_exists(container_name)
            .map_err(|e| {
                Report::new(DMError::IOError)
                    .attach_printable(format!("Failed to create container '{}'", container_name))
                    .attach(e)
            })?;
        self.runtime.block_on(async {
            let mut result = Vec::new();
            let mut stream = self
                .blob_service_client
                .container_client(container_name)
                .list_blobs()
                .into_stream();

            loop {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_millis(500))=> {
                        jerror!("Timeout while listing blobs, returning partial results");
                        return Err(Report::new(DMError::Timeout));
                    }

                    response = stream.next() => {
                        match response {
                            Some(Ok(response)) => {
                                for blob in response.blobs.items.iter() {
                                    if let BlobItem::Blob(blob_item) = blob {
                                        result.push(blob_item.clone());
                                    } else {
                                        jdebug!("Skipping non-blob item in list: {:?}", blob);
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                return Err(Report::new(DMError::IOError)
                                    .attach_printable(format!("Failed to list blobs: {}", e)));
                            }
                            None => break, // No more items in the stream
                        }

                    }
                }
            }

            Ok(result)
        })
    }

    pub fn list_blobs_for_ui(&self, container_name: &str) -> Result<Vec<UiBlob>, DMError> {
        self.create_container_if_not_exists(container_name)
            .map_err(|e| {
                Report::new(DMError::IOError)
                    .attach_printable(format!("Failed to create container '{}'", container_name))
                    .attach(e)
            })?;

        self.runtime.block_on(async {
            let mut result = Vec::new();
            let mut stream = self
                .blob_service_client
                .container_client(container_name)
                .list_blobs()
                .into_stream();

            loop {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                        return Err(Report::new(DMError::Timeout)
                            .attach_printable("Failed to list blobs: timeout after 5s"));
                    }

                    response = stream.next() => {
                        match response {
                            Some(Ok(response)) => {
                                for blob in response.blobs.items.iter() {
                                    if let BlobItem::Blob(blob_item) = blob {
                                        let ui_blob = UiBlob {
                                            name: blob_item.name.clone(),
                                            created_on: {
                                                // Convert Azure's OffsetDateTime to chrono DateTime
                                                let creation_time = blob_item.properties.creation_time;
                                                chrono::DateTime::parse_from_rfc3339(&creation_time.to_string())
                                                    .map(|parsed| parsed.with_timezone(&chrono::Utc))
                                                    .unwrap_or_else(|_| chrono::Utc::now())
                                            },
                                            size: blob_item.properties.content_length,
                                        };
                                        result.push(ui_blob);
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                return Err(Report::new(DMError::IOError)
                                    .attach_printable(format!("Failed to list blobs: {}", e)));
                            }
                            None => break,
                        }
                    }
                }
            }

            // Sort by creation time (newest first)
            result.sort_by(|a, b| b.created_on.cmp(&a.created_on));

            // Limit to 100 items
            if result.len() > 100 {
                result.truncate(100);
            }

            Ok(result)
        })
    }

    pub fn download_blob_to_current_dir(
        &self,
        container_name: &str,
        blob_name: &str,
    ) -> Result<String, DMError> {
        let blob_data = self.get_blob(Some(container_name), blob_name)?;

        let current_dir = std::env::current_dir().map_err(|e| {
            Report::new(DMError::IOError)
                .attach_printable(format!("Failed to get current directory: {}", e))
        })?;

        // Extract the file name from the blob name
        let file_name = blob_name.split('/').last().ok_or_else(|| {
            Report::new(DMError::InvalidData)
                .attach_printable("Blob name does not contain a valid file name")
        })?;
        let file_path = current_dir.join(file_name);

        jdebug!(
            func = "AzuriteStorage::download_blob_to_current_dir()",
            line = line!(),
            message = format!(
                "Writing blob '{}' to file: {}",
                blob_name,
                file_path.display()
            ),
        );

        std::fs::write(&file_path, blob_data).map_err(|e| {
            Report::new(DMError::IOError).attach_printable(format!(
                "Failed to write to '{}' : {}",
                file_path.display(),
                e
            ))
        })?;

        Ok(file_path.to_string_lossy().to_string())
    }

    pub fn get_sas_url(
        &self,
        container_name: &str,
        blob: &str,
        permissions: Option<BlobSasPermissions>,
        valid_duration: Option<std::time::Duration>,
    ) -> Result<String, DMError> {
        // Basic validation to avoid generating SAS for obviously invalid inputs.
        if container_name.trim().is_empty()
            || container_name.contains("..")
            || container_name.contains('\\')
            || container_name.len() > 256
        {
            return Err(
                Report::new(DMError::InvalidData).attach_printable("Invalid container name")
            );
        }

        if blob.contains("..") || blob.contains('\\') || blob.len() > 1024 {
            return Err(Report::new(DMError::InvalidData).attach_printable("Invalid blob name"));
        }

        let blob_client = self
            .blob_service_client
            .container_client(container_name)
            .blob_client(blob);

        let default_permissions = BlobSasPermissions {
            read: true,
            write: false,
            ..Default::default()
        };
        let sas_permissions = permissions.unwrap_or(default_permissions);
        let duration = valid_duration.unwrap_or_else(|| std::time::Duration::from_secs(3600)); // Default: 1 hour

        self.runtime.block_on(async {
            let signature = blob_client
                .shared_access_signature(sas_permissions, OffsetDateTime::now_utc() + duration)
                .await
                .map_err(|e| {
                    Report::new(DMError::IOError)
                        .attach_printable(format!("Failed to generate SAS signature: {}", e))
                })?;

            let sas_url = blob_client
                .generate_signed_blob_url(&signature)
                .map_err(|e| {
                    Report::new(DMError::IOError)
                        .attach_printable(format!("Failed to generate SAS URL: {}", e))
                })?;

            // Do not log or expose SAS tokens in logs. Return the signed URL to the caller.
            Ok(sas_url.to_string())
        })
    }

    pub fn is_sas_url_valid(sas_url: &str) -> bool {
        // Extract the raw 'se' query value from the URL string directly to avoid URL
        // query-pair decoding rules that turn '+' into space (which breaks RFC3339 offsets).
        if let Some(pos) = sas_url.find("se=") {
            let rest = &sas_url[pos + 3..];
            let end = rest.find('&').unwrap_or(rest.len());
            let expire_raw = rest[..end].trim();

            if let Ok(expire_time) = chrono::DateTime::parse_from_rfc3339(expire_raw) {
                let now = chrono::Utc::now();
                return expire_time > now;
            }
        }

        false
    }

    pub fn update_modules(&mut self, container_name: Option<&str>) -> Result<(), DMError> {
        let blobs = self.list_blobs(container_name.unwrap_or("default"))?;

        let mut new_module_info_db = HashMap::new();
        for blob in blobs.iter() {
            let module_info_db = &mut self.module_info_db;
            if let Some((uuid, info)) = module_info_db
                .iter()
                .find(|(_, v)| v.blob_name == blob.name)
            {
                jdebug!(
                    func = "AzuriteStorage::update_modules()",
                    line = line!(),
                    message = format!("Found existing module: {}", info.blob_name)
                );
                let uuid = uuid.clone();
                let mut info = info.clone();

                if info.sas_url.is_empty() || !AzuriteStorage::is_sas_url_valid(&info.sas_url) {
                    let blob_client = self
                        .blob_service_client
                        .container_client(container_name.unwrap_or("default"))
                        .blob_client(&blob.name);

                    if let Ok(url) = self.get_sas_url(
                        container_name.unwrap_or("default"),
                        &blob.name,
                        None,
                        None,
                    ) {
                        info.sas_url = url.as_str().to_string();
                    } else {
                        jerror!(
                            func = "AzuriteStorage::update_modules()",
                            line = line!(),
                            error = format!("Failed to get blob {} url, skip it.", blob.name)
                        );
                        continue; // Skip if URL cannot be generated
                    }
                }
                new_module_info_db.insert(uuid, info);
                continue;
            }

            let blob_client = self
                .blob_service_client
                .container_client(container_name.unwrap_or("default"))
                .blob_client(&blob.name);

            let mut sas_url = String::new();
            if let Ok(url) =
                self.get_sas_url(container_name.unwrap_or("default"), &blob.name, None, None)
            {
                sas_url = url.as_str().to_string();
            } else {
                jerror!(
                    func = "AzuriteStorage::update_modules()",
                    line = line!(),
                    error = format!("Failed to get blob {} url, skip it.", blob.name)
                );
                continue; // Skip if URL cannot be generated
            }

            if let Ok(buf) = self.get_blob(container_name, &blob.name) {
                let mut hasher = Sha256::new();
                hasher.update(&buf);
                let hash = format!("{:x}", hasher.finalize());
                let module_id = UUID::new();
                let module_info = ModuleInfo {
                    id: module_id.clone(),
                    blob_name: blob.name.clone(),
                    container_name: container_name.unwrap_or("default").to_string(),
                    hash,
                    sas_url,
                };
                new_module_info_db.insert(module_id, module_info);
            } else {
                jerror!(
                    func = "AzuriteStorage::update_modules()",
                    line = line!(),
                    error = format!("Failed to get blob {}, skip it.", blob.name)
                );
            }
        }

        self.module_info_db = new_module_info_db;
        self.current_module_id = 0;

        jdebug!(func="AzuriteStorage::update_modules()",
                line = line!(),
                module_info_db = ?self.module_info_db);

        Ok(())
    }

    pub fn module_info_db(&self) -> &HashMap<UUID, ModuleInfo> {
        &self.module_info_db
    }

    pub fn action(&self) -> AzuriteAction {
        self.action
    }

    pub fn set_action(&mut self, action: AzuriteAction) {
        self.action = action;
    }

    pub fn current_module_focus_init(&mut self) {
        self.current_module_id = 0;
    }

    pub fn current_module_focus_down(&mut self) {
        if self.current_module_id < self.module_info_db.len() - 1 {
            self.current_module_id += 1;
        } else {
            self.current_module_id = 0;
        }
    }

    pub fn current_module_focus_up(&mut self) {
        if self.current_module_id == 0 {
            self.current_module_id = self.module_info_db.len() - 1;
        } else {
            self.current_module_id -= 1;
        }
    }

    pub fn current_module(&self) -> Option<&ModuleInfo> {
        self.module_info_db.values().nth(self.current_module_id)
    }

    pub fn current_module_id(&self) -> usize {
        self.current_module_id
    }

    pub fn new_module(&self) -> &str {
        &self.new_module
    }

    pub fn new_module_mut(&mut self) -> &mut String {
        &mut self.new_module
    }

    pub fn scan_upload_containers(&mut self) -> Result<(), DMError> {
        let containers = self.list_containers();
        let mut new_token_providers = HashMap::new();

        for container_name in containers {
            if container_name.starts_with("upload") {
                if let Ok(uuid_str) = container_name
                    .strip_prefix("upload")
                    .unwrap_or("")
                    .trim_start_matches('-')
                    .parse::<uuid::Uuid>()
                {
                    let uuid = match UUID::from(&uuid_str.to_string()) {
                        Ok(uuid) => uuid,
                        Err(_) => continue,
                    };

                    let token_provider = TokenProvider {
                        uuid: uuid.clone(),
                        container: container_name.clone(),
                    };
                    new_token_providers.insert(uuid, token_provider);
                }
            }
        }

        self.token_providers = new_token_providers;
        self.current_token_provider_id = 0;
        Ok(())
    }

    pub fn add_token_provider(&mut self) -> Result<UUID, DMError> {
        let uuid = UUID::new();
        let container_name = format!("upload-{}", uuid.uuid());

        self.create_container(&container_name)?;

        // Create SAS URL with write permissions for short-lived access (1 hour)
        let token_permissions = BlobSasPermissions {
            read: true,
            write: true,
            add: true,
            create: true,
            ..Default::default()
        };
        let one_hour = std::time::Duration::from_secs(3600);

        // Generate a short-lived SAS for the container/blob root. We do not persist the SAS in
        // TokenProvider (we store container name only) but generating it here exercises the
        // generation path and validates the container.
        let _ = self.get_sas_url(&container_name, "", Some(token_permissions), Some(one_hour))?;

        let token_provider = TokenProvider {
            uuid: uuid.clone(),
            container: container_name.clone(),
        };

        self.token_providers.insert(uuid.clone(), token_provider);
        Ok(uuid)
    }

    pub fn remove_token_provider(&mut self, uuid: &UUID) -> Result<(), DMError> {
        if let Some(_) = self.token_providers.remove(uuid) {
            let container_name = format!("upload-{}", uuid.uuid());
            self.delete_container(&container_name)?;

            if self.current_token_provider_id >= self.token_providers.len() {
                self.current_token_provider_id = if self.token_providers.is_empty() {
                    0
                } else {
                    self.token_providers.len() - 1
                };
            }
        }
        Ok(())
    }

    pub fn token_providers(&self) -> &HashMap<UUID, TokenProvider> {
        &self.token_providers
    }

    pub fn current_token_provider(&self) -> Option<&TokenProvider> {
        self.token_providers
            .values()
            .nth(self.current_token_provider_id)
    }

    pub fn current_token_provider_id(&self) -> usize {
        self.current_token_provider_id
    }

    pub fn current_token_provider_focus_init(&mut self) {
        self.current_token_provider_id = 0;
    }

    pub fn current_token_provider_focus_down(&mut self) {
        if self.current_token_provider_id < self.token_providers.len().saturating_sub(1) {
            self.current_token_provider_id += 1;
        } else {
            self.current_token_provider_id = 0;
        }
    }

    pub fn current_token_provider_focus_up(&mut self) {
        if self.current_token_provider_id == 0 {
            self.current_token_provider_id = self.token_providers.len().saturating_sub(1);
        } else {
            self.current_token_provider_id -= 1;
        }
    }

    pub fn get_current_token_provider_by_highlight(&self) -> Option<&UUID> {
        self.token_providers
            .keys()
            .nth(self.current_token_provider_id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azurite_action_default() {
        assert_eq!(AzuriteAction::default(), AzuriteAction::Deploy);
    }

    #[test]
    fn test_ui_blob_creation() {
        let blob = UiBlob {
            name: "test.txt".to_string(),
            created_on: chrono::Utc::now(),
            size: 1024,
        };

        assert_eq!(blob.name, "test.txt");
        assert_eq!(blob.size, 1024);
        assert!(blob.created_on <= chrono::Utc::now());
    }

    #[test]
    fn test_blob_list_state_navigation() {
        use crate::app::ui::ui_token_provider_blobs::BlobListState;
        let mut state = BlobListState::new("test-container".to_string());

        // Test with empty list
        assert_eq!(state.selected_index, 0);
        state.move_up();
        assert_eq!(state.selected_index, 0);
        state.move_down();
        assert_eq!(state.selected_index, 0);

        // Add some test blobs
        state.blobs = vec![
            UiBlob {
                name: "blob1.txt".to_string(),
                created_on: chrono::Utc::now(),
                size: 100,
            },
            UiBlob {
                name: "blob2.txt".to_string(),
                created_on: chrono::Utc::now(),
                size: 200,
            },
            UiBlob {
                name: "blob3.txt".to_string(),
                created_on: chrono::Utc::now(),
                size: 300,
            },
        ];

        // Test navigation
        assert_eq!(state.selected_index, 0);
        state.move_down();
        assert_eq!(state.selected_index, 1);
        state.move_down();
        assert_eq!(state.selected_index, 2);
        state.move_down(); // Should wrap to 0
        assert_eq!(state.selected_index, 0);

        state.move_up(); // Should wrap to last item
        assert_eq!(state.selected_index, 2);
        state.move_up();
        assert_eq!(state.selected_index, 1);
        state.move_up();
        assert_eq!(state.selected_index, 0);

        // Test current_blob
        assert_eq!(state.current_blob().unwrap().name, "blob1.txt");
        state.move_down();
        assert_eq!(state.current_blob().unwrap().name, "blob2.txt");
    }

    #[test]
    fn test_new_module_methods() {
        let mut storage = AzuriteStorage {
            runtime: tokio::runtime::Runtime::new().unwrap(),
            blob_service_client: ClientBuilder::with_location(
                CloudLocation::Emulator {
                    address: "127.0.0.1".to_string(),
                    port: 10000,
                },
                StorageCredentials::access_key(ACCOUNT_NAME, ACCOUNT_KEY),
            )
            .blob_service_client(),
            module_info_db: HashMap::new(),
            current_module_id: 0,
            new_module: "test_module".to_string(),
            action: AzuriteAction::Deploy,
            token_providers: HashMap::new(),
            current_token_provider_id: 0,
        };
        assert_eq!(storage.new_module(), "test_module");
        storage.new_module_mut().push_str("_mut");
        assert_eq!(storage.new_module(), "test_module_mut");
    }

    #[test]
    fn test_set_and_get_action() {
        let mut storage = AzuriteStorage {
            runtime: tokio::runtime::Runtime::new().unwrap(),
            blob_service_client: ClientBuilder::with_location(
                CloudLocation::Emulator {
                    address: "127.0.0.1".to_string(),
                    port: 10000,
                },
                StorageCredentials::access_key(ACCOUNT_NAME, ACCOUNT_KEY),
            )
            .blob_service_client(),
            module_info_db: HashMap::new(),
            current_module_id: 0,
            new_module: String::new(),
            action: AzuriteAction::Deploy,
            token_providers: HashMap::new(),
            current_token_provider_id: 0,
        };
        assert_eq!(storage.action(), AzuriteAction::Deploy);
        storage.set_action(AzuriteAction::Add);
        assert_eq!(storage.action(), AzuriteAction::Add);
    }

    #[test]
    fn test_current_module_id() {
        let storage = AzuriteStorage {
            runtime: tokio::runtime::Runtime::new().unwrap(),
            blob_service_client: ClientBuilder::with_location(
                CloudLocation::Emulator {
                    address: "127.0.0.1".to_string(),
                    port: 10000,
                },
                StorageCredentials::access_key(ACCOUNT_NAME, ACCOUNT_KEY),
            )
            .blob_service_client(),
            module_info_db: HashMap::new(),
            current_module_id: 42,
            new_module: String::new(),
            action: AzuriteAction::Deploy,
            token_providers: HashMap::new(),
            current_token_provider_id: 0,
        };
        assert_eq!(storage.current_module_id(), 42);
    }

    #[test]
    fn test_is_sas_url_valid_future() {
        let expire = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        let url = format!("https://example.com/blob?se={}", expire);
        assert!(AzuriteStorage::is_sas_url_valid(&url));
    }

    #[test]
    fn test_is_sas_url_valid_past() {
        let expire = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let url = format!("https://example.com/blob?se={}", expire);
        assert!(!AzuriteStorage::is_sas_url_valid(&url));
    }

    #[test]
    fn test_token_provider_highlight_and_current() {
        let mut storage = AzuriteStorage {
            runtime: tokio::runtime::Runtime::new().unwrap(),
            blob_service_client: ClientBuilder::with_location(
                CloudLocation::Emulator {
                    address: "127.0.0.1".to_string(),
                    port: 10000,
                },
                StorageCredentials::access_key(ACCOUNT_NAME, ACCOUNT_KEY),
            )
            .blob_service_client(),
            module_info_db: HashMap::new(),
            current_module_id: 0,
            new_module: String::new(),
            action: AzuriteAction::Deploy,
            token_providers: HashMap::new(),
            current_token_provider_id: 0,
        };

        // Initially there are no token providers
        assert!(storage.get_current_token_provider_by_highlight().is_none());

        // Insert two token providers
        let u1 = UUID::new();
        let u2 = UUID::new();
        storage.token_providers.insert(
            u1.clone(),
            TokenProvider {
                uuid: u1.clone(),
                container: format!("upload-{}", u1.uuid()),
            },
        );
        storage.token_providers.insert(
            u2.clone(),
            TokenProvider {
                uuid: u2.clone(),
                container: format!("upload-{}", u2.uuid()),
            },
        );

        // Determine the current highlight index deterministically by collecting the keys
        // in the same iteration order used by get_current_token_provider_by_highlight().
        let keys: Vec<UUID> = storage.token_providers.keys().cloned().collect();
        let pos_u2 = keys
            .iter()
            .position(|k| k == &u2)
            .expect("u2 should be present in token_providers");
        storage.current_token_provider_id = pos_u2;
        assert_eq!(storage.get_current_token_provider_by_highlight(), Some(&u2));
    }
}
