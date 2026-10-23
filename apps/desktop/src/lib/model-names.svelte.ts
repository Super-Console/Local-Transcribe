import { invoke } from "@tauri-apps/api/core";

import type { CatalogModel } from "./types";
import { prettyNameFrom } from "./model-names.js";

/// 清单只拉一次；`$state` 让模板在清单到达后自动重新渲染。
let entries = $state<CatalogModel[] | null>(null);
let requested = false;

export function ensureModelNames(): void {
  if (requested) return;
  requested = true;
  void invoke<CatalogModel[]>("model_catalog")
    .then((catalog) => {
      entries = catalog;
    })
    .catch(() => {
      entries = [];
    });
}

/// 模型显示名的唯一来源：命中清单用清单名，否则去掉 sherpa-onnx- 与日期。
export function prettyName(value: string | null | undefined): string {
  if (!value) return "";
  ensureModelNames();
  return prettyNameFrom(value, entries ?? []);
}
