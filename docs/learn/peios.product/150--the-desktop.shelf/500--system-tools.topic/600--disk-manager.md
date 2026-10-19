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

## What cannot change storage

The forms for erasing a disk, adding or removing a partition, formatting,
mounting, unmounting, changing mount policy, startup configuration, and
ejecting all end by reporting that nothing changed. Passing validation or
confirming one of these forms is not a completed disk operation.

Use the separately documented storage tools when you need to perform an
actual operation. Their risks and access requirements belong to those
tools; do not infer them from this read-only inspection interface.
See `gxwi-disk-manager(1)` for data sources and limitations.
