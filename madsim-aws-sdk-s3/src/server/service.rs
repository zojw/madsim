use crate::input::*;
use crate::model::*;
use crate::output::*;
use bytes::Bytes;
use madsim::rand::{thread_rng, Rng};
use spin::Mutex;
use tracing::debug;

use std::collections::{btree_map::Entry::*, BTreeMap, VecDeque};

use aws_sdk_s3::error::*;

/// A request to s3 server.
#[derive(Debug)]
pub(crate) enum Request {
    CreateMultipartUpload(CreateMultipartUploadInput),
    UploadPart(UploadPartInput),
    CompletedMultipartUpload(CompleteMultipartUploadInput),
    AbortMultipartUpload(AbortMultipartUploadInput),
    GetObject(GetObjectInput),
    PutObject(PutObjectInput),
    DeleteObject(DeleteObjectInput),
    DeleteObjects(DeleteObjectsInput),
    HeadObject(HeadObjectInput),
    ListObjectsV2(ListObjectsV2Input),
    PutBucketLifecycleConfiguration(PutBucketLifecycleConfigurationInput),
    GetBucketLifecycleConfiguration(GetBucketLifecycleConfigurationInput),
}

#[derive(Debug)]
pub struct S3Service {
    inner: Mutex<ServiceInner>,
}

impl S3Service {
    pub fn new() -> Self {
        S3Service {
            inner: Mutex::new(ServiceInner::default()),
        }
    }

    pub async fn create_bucket(&self, name: &str) {
        self.inner.lock().create_bucket(name)
    }

    pub async fn create_multipart_upload(
        &self,
        bucket: String,
        key: String,
    ) -> Result<CreateMultipartUploadOutput, CreateMultipartUploadError> {
        self.inner.lock().create_multipart_upload(bucket, key)
    }

    pub async fn upload_part(
        &self,
        bucket: String,
        key: String,
        body: Bytes,
        content_length: i64,
        part_number: i32,
        upload_id: String,
    ) -> Result<UploadPartOutput, UploadPartError> {
        self.inner
            .lock()
            .upload_part(bucket, key, body, content_length, part_number, upload_id)
    }

    pub async fn complete_multipart_upload(
        &self,
        bucket: String,
        key: String,
        multipart: crate::model::CompletedMultipartUpload,
        upload_id: String,
    ) -> Result<CompleteMultipartUploadOutput, CompleteMultipartUploadError> {
        self.inner
            .lock()
            .complete_multipart_upload(bucket, key, multipart, upload_id)
    }

    pub async fn abort_multipart_upload(
        &self,
        bucket: String,
        key: String,
        upload_id: String,
    ) -> Result<AbortMultipartUploadOutput, AbortMultipartUploadError> {
        self.inner
            .lock()
            .abort_multipart_upload(bucket, key, upload_id)
    }

    pub async fn get_object(
        &self,
        bucket: String,
        key: String,
        range: Option<String>,
        part_number: Option<i32>,
    ) -> Result<GetObjectOutput, GetObjectError> {
        self.inner
            .lock()
            .get_object(bucket, key, range, part_number)
    }

    pub async fn put_object(
        &self,
        bucket: String,
        key: String,
        object: Bytes,
    ) -> Result<PutObjectOutput, PutObjectError> {
        self.inner.lock().put_object(bucket, key, object)
    }

    pub async fn delete_object(
        &self,
        bucket: String,
        key: String,
    ) -> Result<DeleteObjectOutput, DeleteObjectError> {
        self.inner.lock().delete_object(bucket, key)
    }

    pub async fn delete_objects(
        &self,
        bucket: String,
        delete: crate::model::Delete,
    ) -> Result<DeleteObjectsOutput, DeleteObjectsError> {
        self.inner.lock().delete_objects(bucket, delete)
    }

    pub async fn head_object(
        &self,
        bucket: String,
        key: String,
    ) -> Result<HeadObjectOutput, HeadObjectError> {
        self.inner.lock().head_object(bucket, key)
    }

    pub async fn list_objects_v2(
        &self,
        bucket: String,
        prefix: Option<String>,
        continuation_token: Option<String>,
    ) -> Result<ListObjectsV2Output, ListObjectsV2Error> {
        self.inner
            .lock()
            .list_objects_v2(bucket, prefix, continuation_token)
    }

    pub async fn get_bucket_lifecycle_configuration(
        &self,
        bucket: String,
        expected_bucket_owner: Option<String>,
    ) -> Result<GetBucketLifecycleConfigurationOutput, GetBucketLifecycleConfigurationError> {
        self.inner
            .lock()
            .get_bucket_lifecycle_configuration(bucket, expected_bucket_owner)
    }

    pub async fn put_bucket_lifecycle_configuration(
        &self,
        bucket: String,
        lifecycle_configuration: Option<BucketLifecycleConfiguration>,
        expected_bucket_owner: Option<String>,
    ) -> Result<PutBucketLifecycleConfigurationOutput, PutBucketLifecycleConfigurationError> {
        self.inner.lock().put_bucket_lifecycle_configuration(
            bucket,
            lifecycle_configuration.unwrap_or(BucketLifecycleConfiguration {
                rules: Some(Vec::new()),
            }),
            expected_bucket_owner,
        )
    }
}

#[derive(Debug, Default)]
struct ServiceInner {
    /// (bucket, key) -> Object
    storage: BTreeMap<String, BTreeMap<String, Object>>,

    /// (bucket) -> LifecycleRules
    lifecycle: BTreeMap<String, Vec<LifecycleRule>>,
}

#[derive(Debug, Default)]
struct Object {
    body: Bytes,

    completed: bool,

    /// upload_id -> parts
    parts: BTreeMap<String, Vec<ObjectPart>>,

    last_modified: Option<crate::types::DateTime>,

    content_length: i64,
}

#[derive(Debug, Default)]
struct ObjectPart {
    part_number: i32,
    body: Bytes,
    e_tag: String,
}

#[allow(clippy::result_large_err)]
impl ServiceInner {
    fn create_bucket(&mut self, name: &str) {
        debug!(name, "create_bucket");
        if self.storage.contains_key(name) {
            panic!("bucket already exists: {name}");
        }
        self.storage.insert(name.to_string(), Default::default());
    }

    fn create_multipart_upload(
        &mut self,
        bucket: String,
        key: String,
    ) -> Result<CreateMultipartUploadOutput, CreateMultipartUploadError> {
        debug!(bucket, key, "create_multipart_upload");
        let object = self
            .storage
            .get_mut(&bucket)
            .ok_or_else(|| CreateMultipartUploadError::unhandled(no_such_bucket(&bucket)))?
            .entry(key)
            .or_default();

        loop {
            let upload_id = thread_rng().gen::<u32>().to_string();
            if object.parts.contains_key(&upload_id) {
                continue;
            } else {
                object.parts.insert(upload_id.clone(), Default::default());
                return Ok(CreateMultipartUploadOutput {
                    upload_id: Some(upload_id),
                });
            }
        }
    }

    fn upload_part(
        &mut self,
        bucket: String,
        key: String,
        body: Bytes,
        _content_length: i64,
        part_number: i32,
        upload_id: String,
    ) -> Result<UploadPartOutput, UploadPartError> {
        debug!(bucket, key, upload_id, part_number, "upload_part");
        let object = self
            .storage
            .get_mut(&bucket)
            .ok_or_else(|| UploadPartError::unhandled(no_such_bucket(&bucket)))?
            .get_mut(&key)
            .ok_or_else(|| UploadPartError::unhandled(no_such_key(&key)))?;

        let parts = object
            .parts
            .get_mut(&upload_id)
            .ok_or_else(|| UploadPartError::unhandled(no_such_upload(&upload_id)))?;

        let e_tag = thread_rng().gen::<u32>().to_string();
        let part = ObjectPart {
            part_number,
            body,
            e_tag: e_tag.clone(),
        };
        parts.push(part);

        let e_tag = Some(e_tag);
        Ok(UploadPartOutput { e_tag })
    }

    fn complete_multipart_upload(
        &mut self,
        bucket: String,
        key: String,
        multipart: crate::model::CompletedMultipartUpload,
        upload_id: String,
    ) -> Result<CompleteMultipartUploadOutput, CompleteMultipartUploadError> {
        debug!(bucket, key, upload_id, "complete_multipart_upload");
        let object = self
            .storage
            .get_mut(&bucket)
            .ok_or_else(|| CompleteMultipartUploadError::unhandled(no_such_bucket(&bucket)))?
            .get_mut(&key)
            .ok_or_else(|| CompleteMultipartUploadError::unhandled(no_such_key(&key)))?;

        let parts = object
            .parts
            .get_mut(&upload_id)
            .ok_or_else(|| CompleteMultipartUploadError::unhandled(no_such_upload(&upload_id)))?;

        if let Some(mut multipart) = multipart.parts {
            multipart.sort_by_key(|part| part.part_number);
            let mut selection_idx = vec![];
            for completed_part in multipart {
                for (idx, part) in parts.iter().enumerate() {
                    if part.part_number == completed_part.part_number {
                        if let Some(e_tag) = &completed_part.e_tag {
                            if e_tag == &part.e_tag {
                                selection_idx.push(idx);
                                break;
                            }
                        } else {
                            selection_idx.push(idx);
                            break;
                        }
                    }
                }
            }

            selection_idx.sort();
            let mut selection_idx = VecDeque::from(selection_idx);
            let mut body = vec![];
            let parts = object.parts.remove(&upload_id).unwrap();

            for (idx, part) in parts.into_iter().enumerate() {
                if let Some(next_idx) = selection_idx.front() {
                    if *next_idx != idx {
                        continue;
                    } else {
                        body.extend(part.body);
                        selection_idx.pop_front();
                    }
                } else {
                    break;
                }
            }

            object.body = body.into();
            object.completed = true;
            object.parts.remove(&upload_id);

            Ok(CompleteMultipartUploadOutput {})
        } else {
            object
                .parts
                .remove(&upload_id)
                .expect("empty complete multipart request, remove upload_id failed");
            Ok(CompleteMultipartUploadOutput {})
        }
    }

    fn abort_multipart_upload(
        &mut self,
        bucket: String,
        key: String,
        upload_id: String,
    ) -> Result<AbortMultipartUploadOutput, AbortMultipartUploadError> {
        debug!(bucket, key, upload_id, "abort_multipart_upload");
        let object = self
            .storage
            .get_mut(&bucket)
            .ok_or_else(|| AbortMultipartUploadError::unhandled(no_such_bucket(&bucket)))?
            .get_mut(&key)
            .ok_or_else(|| AbortMultipartUploadError::unhandled(no_such_key(&key)))?;

        object
            .parts
            .remove(&upload_id)
            .ok_or_else(|| AbortMultipartUploadError::unhandled(no_such_upload(&upload_id)))?;
        Ok(AbortMultipartUploadOutput {})
    }

    fn get_object(
        &self,
        bucket: String,
        key: String,
        range: Option<String>,
        part_number: Option<i32>,
    ) -> Result<GetObjectOutput, GetObjectError> {
        debug!(bucket, key, range, part_number, "get_object");
        let object = self
            .storage
            .get(&bucket)
            .ok_or_else(|| GetObjectError::unhandled(no_such_bucket(&bucket)))?
            .get(&key)
            .ok_or_else(|| {
                GetObjectError::new(GetObjectErrorKind::NoSuchKey(no_such_key(&key)), meta())
            })?;
        if !object.completed {
            return Err(GetObjectError::new(
                GetObjectErrorKind::NoSuchKey(no_such_key(&key)),
                meta(),
            ));
        }

        if let Some(range) = range {
            let invalid_range = || GetObjectError::unhandled(format!("invalid range: {range}"));
            // https://www.rfc-editor.org/rfc/rfc9110.html#name-range
            let mut split = range.split('=');
            let range_unit = split.next().ok_or_else(invalid_range)?;

            if range_unit != "bytes" {
                return Err(GetObjectError::unhandled(format!(
                    "unsupported range unit: {range_unit}"
                )));
            }

            let range_set = split.next().ok_or_else(invalid_range)?;

            let (begin_str, end_str) = range_set.split_once('-').ok_or_else(invalid_range)?;
            let begin_pos = if begin_str.is_empty() {
                None
            } else {
                Some(begin_str.parse::<usize>().map_err(|_| invalid_range())?)
            };
            let end_pos = if end_str.is_empty() {
                None
            } else {
                Some(end_str.parse::<usize>().map_err(|_| invalid_range())?)
            };
            let body = match (begin_pos, end_pos) {
                (Some(begin), Some(end)) => object.body.slice(begin..=end),
                (Some(begin), None) => object.body.slice(begin..),
                (None, Some(len)) => object.body.slice(object.body.len() - len..),
                (None, None) => object.body.slice(..),
            };

            Ok(GetObjectOutput { body: body.into() })
        } else if let Some(part_number) = part_number {
            if part_number < 0 || part_number as usize >= object.body.len() {
                return Err(GetObjectError::unhandled(format!(
                    "invalid part number: {part_number}"
                )));
            };
            let _part_number = part_number as usize;
            todo!("get object by part number");
        } else {
            Ok(GetObjectOutput {
                body: object.body.clone().into(),
            })
        }
    }

    fn put_object(
        &mut self,
        bucket: String,
        key: String,
        body: Bytes,
    ) -> Result<PutObjectOutput, PutObjectError> {
        debug!(bucket, key, len = body.len(), "put_object");
        let object = self
            .storage
            .get_mut(&bucket)
            .ok_or_else(|| PutObjectError::unhandled(no_such_bucket(&bucket)))?
            .entry(key)
            .or_default();

        object.body = body;
        object.completed = true;

        Ok(PutObjectOutput {})
    }

    fn delete_object(
        &mut self,
        bucket: String,
        key: String,
    ) -> Result<DeleteObjectOutput, DeleteObjectError> {
        debug!(bucket, key, "delete_object");
        let object = self
            .storage
            .get_mut(&bucket)
            .ok_or_else(|| DeleteObjectError::unhandled(no_such_bucket(&bucket)))?
            .entry(key);

        if let Occupied(mut o) = object {
            if o.get().completed {
                if o.get().parts.is_empty() {
                    o.remove();
                } else {
                    let object = o.get_mut();
                    object.completed = false;
                    object.body.clear();
                }
            }
        }
        Ok(DeleteObjectOutput {})
    }

    fn delete_objects(
        &mut self,
        bucket: String,
        delete: crate::model::Delete,
    ) -> Result<DeleteObjectsOutput, DeleteObjectsError> {
        debug!(bucket, "delete_objects");
        let bucket = self
            .storage
            .get_mut(&bucket)
            .ok_or_else(|| DeleteObjectsError::unhandled(no_such_bucket(&bucket)))?;

        let mut output = DeleteObjectsOutput::builder();
        let Some(delete) = delete.objects else {
            return Ok(output.build());
        };

        for key in delete.into_iter().flat_map(|i| i.key) {
            if let Occupied(mut o) = bucket.entry(key.clone()) {
                if o.get().completed {
                    if o.get().parts.is_empty() {
                        o.remove();
                    } else {
                        let object = o.get_mut();
                        object.completed = false;
                        object.body.clear();
                    }
                }
            }
            output = output.deleted(DeletedObject::builder().key(key).build());
        }
        Ok(output.build())
    }

    fn head_object(
        &self,
        bucket: String,
        key: String,
    ) -> Result<HeadObjectOutput, HeadObjectError> {
        debug!(bucket, key, "head_object");
        let object = self
            .storage
            .get(&bucket)
            .ok_or_else(|| HeadObjectError::unhandled(no_such_bucket(&bucket)))?
            .get(&key)
            .ok_or_else(|| {
                HeadObjectError::new(HeadObjectErrorKind::NotFound(not_found(&key)), meta())
            })?;

        if !object.completed {
            return Err(HeadObjectError::new(
                HeadObjectErrorKind::NotFound(not_found(&key)),
                meta(),
            ));
        }
        let last_modified = object.last_modified;
        let content_length = object.content_length;
        Ok(HeadObjectOutput {
            last_modified,
            content_length,
        })
    }

    fn list_objects_v2(
        &mut self,
        bucket: String,
        prefix: Option<String>,
        _continuation_token: Option<String>,
    ) -> Result<ListObjectsV2Output, ListObjectsV2Error> {
        debug!(bucket, prefix, "list_objects_v2");
        let bucket = self.storage.get_mut(&bucket).ok_or_else(move || {
            ListObjectsV2Error::new(
                ListObjectsV2ErrorKind::NoSuchBucket(no_such_bucket(&bucket)),
                meta(),
            )
        })?;

        if let Some(prefix) = prefix {
            let objects = bucket
                .iter()
                .filter(|(key, object)| key.starts_with(&prefix) && object.completed)
                .map(|(key, object)| crate::model::Object {
                    key: Some(key.clone()),
                    last_modified: None,
                    e_tag: None,
                    size: object.content_length,
                })
                .collect();
            Ok(ListObjectsV2Output {
                is_truncated: false,
                contents: Some(objects),
                next_continuation_token: None,
            })
        } else {
            Ok(ListObjectsV2Output {
                is_truncated: false,
                contents: Some(
                    bucket
                        .iter()
                        .map(|(key, object)| crate::model::Object {
                            key: Some(key.clone()),
                            last_modified: None,
                            e_tag: None,
                            size: object.content_length,
                        })
                        .collect(),
                ),
                next_continuation_token: None,
            })
        }
    }

    fn get_bucket_lifecycle_configuration(
        &mut self,
        bucket: String,
        _expected_bucket_owner: Option<String>,
    ) -> Result<GetBucketLifecycleConfigurationOutput, GetBucketLifecycleConfigurationError> {
        debug!(bucket, "get_bucket_lifecycle_configuration");
        let lifecycle = match self.lifecycle.entry(bucket) {
            Vacant(v) => {
                v.insert(Vec::new());
                Vec::new()
            }
            Occupied(o) => o.get().clone(),
        };

        Ok(GetBucketLifecycleConfigurationOutput {
            rules: Some(lifecycle),
        })
    }

    fn put_bucket_lifecycle_configuration(
        &mut self,
        bucket: String,
        lifecycle_configuration: BucketLifecycleConfiguration,
        _expected_bucket_owner: Option<String>,
    ) -> Result<PutBucketLifecycleConfigurationOutput, PutBucketLifecycleConfigurationError> {
        debug!(bucket, "put_bucket_lifecycle_configuration");
        self.lifecycle
            .insert(bucket, lifecycle_configuration.rules.unwrap_or_default());

        Ok(PutBucketLifecycleConfigurationOutput {})
    }
}

/// Returns a `NoSuchBucket` error.
fn no_such_bucket(bucket: &str) -> NoSuchBucket {
    NoSuchBucket::builder().message(bucket).build()
}

/// Returns a `NoSuchKey` error.
fn no_such_key(key: &str) -> NoSuchKey {
    NoSuchKey::builder().message(key).build()
}

/// Returns a `NoSuchUpload` error.
fn no_such_upload(upload_id: &str) -> NoSuchUpload {
    NoSuchUpload::builder().message(upload_id).build()
}

/// Returns a `NotFound` error.
fn not_found(content: &str) -> NotFound {
    NotFound::builder().message(content).build()
}

/// Returns a meta.
fn meta() -> aws_smithy_types::error::Error {
    aws_smithy_types::error::Error::builder().build()
}
