---
title: Arranging Metric Dashboards
type: how-to
description: Choose a metric and transformation that answer the intended question.
---

Event Viewer's Metrics tab lets you arrange charts into personal
dashboards. A chart's time range, transformation, aggregation, and grouping
change the question it asks eventd.

## Before you start

You need read access to the metric and permission to save your dashboard
preferences if they should persist. Inspect what the metric measures
before choosing its chart options.

## Arrange a chart

1. Open **Metrics** and select or create the intended dashboard. Without
   saved dashboards, the viewer shows eventd health.
2. Add the relevant metric and choose the time range.
3. Select the transformation and aggregation suited to its meaning. A
   cumulative counter often needs a rate or delta; a measured level can
   use its recorded samples.
4. Choose whether series are combined or displayed separately, then save
   the arrangement.

## Check the result

Check the chart's units, range, and series labels. A combined series can
hide differences between its individual sources. A gap can indicate no
sample or inaccessible data, not a measurement of zero.

Dashboard preferences allow up to 64 dashboards and 32 charts per
dashboard. Malformed saved entries are skipped. Exact storage syntax is
owned by `gxwi-event-viewer.regman`; eventd owns query and metric semantics.
