---
title: Performance Views
type: concept
description: Interpret Task Manager resource graphs and incomplete process details.
---

Task Manager's performance views describe recent activity on the machine.
They help locate a resource bottleneck, but a graph is not a persistent
monitoring history and does not explain a workload's cause on its own.

**Performance** groups processor, memory, disk, and network measurements.
Select an item to see its graph for the last two minutes. Data is sampled
every two seconds and discarded when the window closes.

Process CPU share is measured against the whole machine between samples,
so it is available from the second sample. **Each processor** shows
logical processors separately. Load averages describe runnable or waiting
threads over their reported intervals rather than the percentage shown by
the CPU graph.

**Available** memory estimates what programs can receive; cache is not all
unavailable memory. Disk graphs show active time and transfer rates.
Network graphs show transfer rates, not whether a remote service is
reachable. Use Network Manager for addresses, routes, and name resolution.

The process list and **Jobs** add context. A job is work submitted to
peinit outside a persistent service definition. Job visibility is checked
separately from process visibility. A process can therefore be shown
without details of the job it belongs to.

Do not infer a machine-wide absence from a filtered view. Task Manager
reports access restrictions and protected processes; compare those notes
with the list before drawing conclusions. Use Event Viewer when you need
recorded events or retained metrics rather than this window's samples.
