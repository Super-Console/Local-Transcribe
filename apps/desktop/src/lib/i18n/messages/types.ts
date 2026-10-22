/**
 * 一条文案：普通字符串，或按复数类别取值的对象
 * （类别由 Intl.PluralRules 给出：one/other/many…，缺类别时回退 other）。
 * 复数对象用于 count 参与的文案，如模型数、历史条数。
 */
export type Message = string | Record<string, string>;
