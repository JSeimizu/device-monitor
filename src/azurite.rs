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
    std::collections::HashMap,
    std::io::Read,
};

const ACCOUNT_NAME: &str = "devstoreaccount1";
const ACCOUNT_KEY: &str =
    "Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AzuriteAction {
    Add,

    #[default]
    Deploy,
}

pub struct AzuriteStorage {
    runtime: tokio::runtime::Runtime,
    blob_service_client: BlobServiceClient,
    module_info_db: HashMap<UUID, ModuleInfo>,
    current_module_id: usize,
    new_module: String,
    action: AzuriteAction,
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

        // Ensure the container exists before uploading the blob
        {
            if !self.is_container_exists(container_name) {
                self.create_container(container_name).map_err(|e| {
                    Report::new(DMError::IOError)
                        .attach_printable(format!("Failed to create {} container", container_name))
                        .attach(e)
                })?;
            }
        }

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

    pub fn get_sas_url(&self, container_name: &str, blob: &str) -> Result<String, DMError> {
        let blob_client = self
            .blob_service_client
            .container_client(container_name)
            .blob_client(blob);

        self.runtime.block_on(async {
            let signature = blob_client
                .shared_access_signature(
                    BlobSasPermissions {
                        read: true,
                        write: false,
                        ..Default::default()
                    },
                    OffsetDateTime::now_utc() + std::time::Duration::from_secs(3600), // 1 hour
                )
                .await
                .map_err(|e| {
                    Report::new(DMError::IOError).attach_printable(format!(
                        "Failed to generate SAS URL for blob '{}': {}",
                        blob, e
                    ))
                })?;

            let sas_url = blob_client
                .generate_signed_blob_url(&signature)
                .map_err(|e| {
                    Report::new(DMError::IOError).attach_printable(format!(
                        "Failed to generate SAS URL for blob '{}': {}",
                        blob, e
                    ))
                })?;

            Ok(sas_url.to_string())
        })
    }

    pub fn is_sas_url_valid(sas_url: &str) -> bool {
        if let Ok(url) = url::Url::parse(sas_url) {
            if let Some(expire) = url
                .query_pairs()
                .find(|(key, _)| key == "se")
                .map(|(_, value)| value.to_string())
            {
                if let Ok(expire_time) = chrono::DateTime::parse_from_rfc3339(&expire) {
                    let now = chrono::Utc::now();
                    return expire_time > now;
                }
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

                    if let Ok(url) =
                        self.get_sas_url(container_name.unwrap_or("default"), &blob.name)
                    {
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
            if let Ok(url) = self.get_sas_url(container_name.unwrap_or("default"), &blob.name) {
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
}
