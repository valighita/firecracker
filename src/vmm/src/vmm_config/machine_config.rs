// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::cpu_config::templates::{CpuTemplateType, CustomCpuTemplate, StaticCpuTemplate};

/// The default memory size of the VM, in MiB.
pub const DEFAULT_MEM_SIZE_MIB: usize = 128;
/// Firecracker aims to support small scale workloads only, so limit the maximum
/// vCPUs supported.
pub const MAX_SUPPORTED_VCPUS: u8 = 32;

/// Errors associated with configuring the microVM.
#[rustfmt::skip]
#[derive(Debug, thiserror::Error, displaydoc::Display, PartialEq, Eq)]
pub enum VmConfigError {
    /// The memory size (MiB) is smaller than the previously set balloon device target size.
    IncompatibleBalloonSize,
    /// The memory size (MiB) is invalid.
    InvalidMemorySize,
    /// The number of vCPUs must be greater than 0, less than {MAX_SUPPORTED_VCPUS:} and must be 1 or an even number if SMT is enabled.
    InvalidVcpuCount,
    /// Could not get the configuration of the previously installed balloon device to validate the memory size.
    InvalidVmState,
    /// Enabling simultaneous multithreading is not supported on aarch64.
    #[cfg(target_arch = "aarch64")]
    SmtNotSupported,
}

/// Struct used in PUT `/machine-config` API call.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MachineConfig {
    /// Number of vcpu to start.
    pub vcpu_count: u8,
    /// The memory size in MiB.
    pub mem_size_mib: usize,
    /// Enables or disabled SMT.
    #[serde(default)]
    pub smt: bool,
    /// A CPU template that it is used to filter the CPU features exposed to the guest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_template: Option<StaticCpuTemplate>,
    /// Enables or disables dirty page tracking. Enabling allows incremental snapshots.
    #[serde(default)]
    pub track_dirty_pages: bool,
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self::from(&VmConfig::default())
    }
}

/// Struct used in PATCH `/machine-config` API call.
/// Used to update `VmConfig` in `VmResources`.
/// This struct mirrors all the fields in `MachineConfig`.
/// All fields are optional, but at least one needs to be specified.
/// If a field is `Some(value)` then we assume an update is requested
/// for that field.
#[derive(Clone, Default, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MachineConfigUpdate {
    /// Number of vcpu to start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcpu_count: Option<u8>,
    /// The memory size in MiB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mem_size_mib: Option<usize>,
    /// Enables or disabled SMT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smt: Option<bool>,
    /// A CPU template that it is used to filter the CPU features exposed to the guest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_template: Option<StaticCpuTemplate>,
    /// Enables or disables dirty page tracking. Enabling allows incremental snapshots.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_dirty_pages: Option<bool>,
}

impl MachineConfigUpdate {
    /// Checks if the update request contains any data.
    /// Returns `true` if all fields are set to `None` which means that there is nothing
    /// to be updated.
    pub fn is_empty(&self) -> bool {
        self == &Default::default()
    }
}

impl From<MachineConfig> for MachineConfigUpdate {
    fn from(cfg: MachineConfig) -> Self {
        MachineConfigUpdate {
            vcpu_count: Some(cfg.vcpu_count),
            mem_size_mib: Some(cfg.mem_size_mib),
            smt: Some(cfg.smt),
            cpu_template: cfg.cpu_template,
            track_dirty_pages: Some(cfg.track_dirty_pages),
        }
    }
}

/// Configuration of the microvm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmConfig {
    /// Number of vcpu to start.
    pub vcpu_count: u8,
    /// The memory size in MiB.
    pub mem_size_mib: usize,
    /// Enables or disabled SMT.
    pub smt: bool,
    /// A CPU template that it is used to filter the CPU features exposed to the guest.
    pub cpu_template: Option<CpuTemplateType>,
    /// Enables or disables dirty page tracking. Enabling allows incremental snapshots.
    pub track_dirty_pages: bool,
}

impl VmConfig {
    /// Sets cpu tempalte field to `CpuTemplateType::Custom(cpu_template)`.
    pub fn set_custom_cpu_template(&mut self, cpu_template: CustomCpuTemplate) {
        self.cpu_template = Some(CpuTemplateType::Custom(cpu_template));
    }

    /// Updates [`VmConfig`] with [`MachineConfigUpdate`].
    /// Mapping for cpu template update:
    /// StaticCpuTemplate::None -> None
    /// StaticCpuTemplate::Other -> Some(CustomCpuTemplate::Static(Other)),
    /// Returns the updated `VmConfig` object.
    pub fn update(&self, update: &MachineConfigUpdate) -> Result<VmConfig, VmConfigError> {
        let vcpu_count = update.vcpu_count.unwrap_or(self.vcpu_count);

        let smt = update.smt.unwrap_or(self.smt);

        #[cfg(target_arch = "aarch64")]
        if smt {
            return Err(VmConfigError::SmtNotSupported);
        }

        if vcpu_count == 0 || vcpu_count > MAX_SUPPORTED_VCPUS {
            return Err(VmConfigError::InvalidVcpuCount);
        }

        // If SMT is enabled or is to be enabled in this call
        // only allow vcpu count to be 1 or even.
        if smt && vcpu_count > 1 && vcpu_count % 2 == 1 {
            return Err(VmConfigError::InvalidVcpuCount);
        }

        let mem_size_mib = update.mem_size_mib.unwrap_or(self.mem_size_mib);

        if mem_size_mib == 0 {
            return Err(VmConfigError::InvalidMemorySize);
        }

        let cpu_template = match update.cpu_template {
            None => self.cpu_template.clone(),
            Some(StaticCpuTemplate::None) => None,
            Some(other) => Some(CpuTemplateType::Static(other)),
        };

        Ok(VmConfig {
            vcpu_count,
            mem_size_mib,
            smt,
            cpu_template,
            track_dirty_pages: update.track_dirty_pages.unwrap_or(self.track_dirty_pages),
        })
    }
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            vcpu_count: 1,
            mem_size_mib: DEFAULT_MEM_SIZE_MIB,
            smt: false,
            cpu_template: None,
            track_dirty_pages: false,
        }
    }
}

impl From<&VmConfig> for MachineConfig {
    fn from(value: &VmConfig) -> Self {
        Self {
            vcpu_count: value.vcpu_count,
            mem_size_mib: value.mem_size_mib,
            smt: value.smt,
            cpu_template: value.cpu_template.as_ref().map(|template| template.into()),
            track_dirty_pages: value.track_dirty_pages,
        }
    }
}
