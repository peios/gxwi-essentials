---
title: Disk Manager
type: concept
description: Inspect storage while recognizing the unsupported mutation path.
---

Disk Manager displays disks, partitions, mounts, and startup mounts. Use it
to inspect the machine's storage layout. Its change forms do not modify
storage because the privileged disk-service backend is not implemented.

## What is shown

A disk page lists its model, capacity, device, partition table, and
partitions. The bar represents each partition's share of the disk and
marks free space. Opening a partition shows available filesystem, label,
identifier, usage, and mount information.

**Mounts** lists active mounts, their options, and available access-policy
information. **At Startup** shows configured startup mounts. StrataFS
views include their constituent strata when readable.

Disk Manager samples every two seconds. It reads block information, the
kernel mount table, filesystem statistics, and permitted policy data.
Some raw-device and policy details need additional privileges. Unreadable
information is marked rather than invented.

Before choosing a separate command that changes storage, compare the device,
model, serial number, exact size, partition layout, and mount points.
**Not Readable** and **Contents unknown** report missing visibility; neither
means that a disk is empty or unformatted. Nonadministrative accounts can
still see kernel-listed partitions even when raw-device details are refused.

A later mount can cover an earlier mount at the same point or an ancestor.
Inspect the mount hierarchy when a filesystem seems to have disappeared.
StrataFS lists strata in precedence order. Startup entries identified by UUID
are distinct from system-managed mounts; displaying either does not enable
editing it.

## Reading a missing-permissions policy

The labels describe how the kernel handles a missing security descriptor:

| Label | Policy | Interpretation |
|---|---|---|
| Deny Access | `deny-missing` | Deny descriptor-based access until a valid descriptor is supplied |
| Give Permissions, Not Saved | `synth-ephemeral` | Derive a descriptor and retain it in memory |
| Give Permissions and Save Them | `synth-persist` | Derive a descriptor and schedule persistence |
| Not Managed | `unmanaged` | No filesystem access-control check from this policy; other kernel checks still apply |

An ephemeral descriptor can remain cached with the inode. Do not interpret
the UI description as a promise to derive a fresh descriptor on every open.
Corrupt stored descriptors are not equivalent to absent descriptors and are
not repaired by choosing synthesis. A filesystem without descriptor storage
cannot persist the synthesized result.

The mount's permissions view describes its fallback template, not the access
rules of every object already on that filesystem. Check the actual object
when diagnosing an access refusal.

## What cannot change storage

The forms for erasing a disk, adding or removing a partition, formatting,
mounting, unmounting, changing mount policy, startup configuration, and
ejecting all end by reporting that nothing changed. Passing validation or
confirming one of these forms is not a completed disk operation.

Use the separately documented storage tools when you need to perform an
actual operation. Their risks and access requirements belong to those
tools; do not infer them from this read-only inspection interface.
See `gxwi-disk-manager(1)` for data sources and limitations.
