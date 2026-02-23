#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
音频质量分析器 v4.0.1 (重构优化版)
提供音频质量分析和报告生成功能

主要功能：
- 读取Rust生成的JSON分析数据
- 执行质量评估算法
- 生成详细的CSV报告
- 支持多种质量指标和阈值配置

作者: Audio Analyzer Team
版本: 4.0.1
"""

from __future__ import annotations

import argparse
import csv
import json
import logging
import os
import sys
import time
import warnings
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Union

# PyInstaller兼容性修复
if getattr(sys, "frozen", False):
    import multiprocessing

    multiprocessing.freeze_support()

    if sys.platform == "darwin":
        bundle_dir = getattr(
            sys, "_MEIPASS", os.path.dirname(os.path.abspath(__file__))
        )
        os.environ["DYLD_LIBRARY_PATH"] = (
            bundle_dir + ":" + os.environ.get("DYLD_LIBRARY_PATH", "")
        )

# 多进程支持
try:
    import multiprocessing

    if sys.platform in ["win32", "darwin"]:
        if __name__ == "__main__":
            multiprocessing.set_start_method("spawn", force=True)
except (ImportError, RuntimeError):
    pass

# 数据处理库
try:
    import numpy as np
    import pandas as pd

    HAS_PANDAS = True
except ImportError:
    HAS_PANDAS = False
    print("警告: 未安装pandas和numpy，某些功能可能受限")

    # 提供基本的替代实现
    class pd:
        class errors:
            PerformanceWarning = UserWarning

        @staticmethod
        def read_json(path):
            with open(path, "r", encoding="utf-8") as f:
                return json.load(f)

    class np:
        @staticmethod
        def nan():
            return float("nan")


try:
    from tqdm import tqdm

    HAS_TQDM = True
except ImportError:
    HAS_TQDM = False

    class tqdm:
        def __init__(self, total=None, desc="", bar_format=None):
            self.total = total
            self.desc = desc
            self.current = 0
            print(f"{desc} 开始...")

        def update(self, n=1):
            self.current += n
            if self.total:
                progress = (self.current / self.total) * 100
                print(f"{self.desc} 进度: {progress:.1f}%")

        def set_postfix_str(self, s):
            print(f"  {s}")

        def __enter__(self):
            return self

        def __exit__(self, *args):
            print(f"{self.desc} 完成!")


performance_warning = getattr(getattr(pd, "errors", object), "PerformanceWarning", UserWarning)
warnings.filterwarnings("ignore", category=performance_warning)
warnings.filterwarnings("ignore", category=UserWarning)

logging.basicConfig(level=logging.INFO, format="%(message)s")
logger = logging.getLogger(__name__)
APP_VERSION = "4.0.1"


@dataclass
class QualityThresholds:
    """
    音频质量评估阈值配置类

    定义了用于音频质量分析的各种阈值参数。这些阈值基于音频工程的最佳实践
    和行业标准，用于识别音频文件中的各种质量问题。

    属性说明：
    - spectrum_*_threshold: 频谱分析阈值，用于检测伪造、处理痕迹等
    - lra_*: LRA（响度范围）相关阈值，用于评估动态范围
    - peak_*: 峰值相关阈值，用于检测削波和过载
    """

    # 频谱分析阈值 (dB)
    spectrum_fake_threshold: float = -85.0  # 伪造检测阈值：低于此值可能是伪造/升频
    spectrum_processed_threshold: float = -80.0  # 处理检测阈值：低于此值可能经过处理
    spectrum_good_threshold: float = -70.0  # 良好阈值：高于此值认为频谱完整

    # LRA（响度范围）阈值 (LU - Loudness Units)
    lra_poor_max: float = 3.0  # 差劲最大值：低于此值为严重压缩
    lra_low_max: float = 6.0  # 低动态最大值：低于此值为低动态
    lra_excellent_min: float = 8.0  # 优秀最小值：此范围内为理想动态
    lra_excellent_max: float = 12.0  # 优秀最大值：此范围内为理想动态
    lra_acceptable_max: float = 15.0  # 可接受最大值：高于此值开始过高
    lra_too_high: float = 20.0  # 过高阈值：高于此值需要压缩处理

    # 峰值阈值
    peak_clipping_db: float = -0.1  # 削波检测阈值 (dB)：接近0dB为削波风险
    peak_clipping_linear: float = 0.999  # 削波检测阈值（线性）：接近1.0为削波风险
    peak_good_db: float = -6.0  # 良好峰值阈值 (dB)：低于此值为安全
    peak_medium_db: float = -3.0  # 中等峰值阈值 (dB)：此值以上需要注意


class AudioQualityAnalyzer:
    """高性能音频质量分析器（PyInstaller兼容版 - 保持原始评分算法）"""

    def __init__(self):
        self.thresholds = QualityThresholds()
        self.stats = {"total_files": 0, "processed_files": 0, "processing_time": 0.0}

    def _safe_fillna(self, series, value=0):
        """安全的fillna操作"""
        try:
            return series.fillna(value)
        except Exception:
            return series.replace([np.nan, None], value)

    def _map_to_score_vectorized(
        self,
        values: pd.Series,
        in_min: float,
        in_max: float,
        out_min: float = 0,
        out_max: float = 1,
    ) -> pd.Series:
        """原始的分数映射函数 - 保持不变"""
        values = self._safe_fillna(values, 0)
        values = np.clip(values, in_min, in_max)
        if in_max == in_min:
            return pd.Series([out_min] * len(values))
        return out_min + (values - in_min) * (out_max - out_min) / (in_max - in_min)

    def _analyze_row_vectorized(self, df: pd.DataFrame) -> Tuple[pd.Series, pd.Series]:
        """原始的状态分析函数 - 保持完全不变"""
        status_series = pd.Series(["质量良好"] * len(df))
        notes_series = pd.Series([""] * len(df))

        critical_fields = ["rmsDbAbove18k", "lra"]
        peak_field = None
        if "peakAmplitudeDb" in df.columns:
            peak_field = "peakAmplitudeDb"
            critical_fields.append("peakAmplitudeDb")
        elif "peakAmplitude" in df.columns:
            peak_field = "peakAmplitude"
            critical_fields.append("peakAmplitude")

        missing_counts = pd.Series([0] * len(df))
        missing_fields_list = []

        for field in critical_fields:
            if field in df.columns:
                field_missing = df[field].isna() | (df[field] == 0.0)
                missing_counts += field_missing.astype(int)
                for idx in df[field_missing].index:
                    if idx not in missing_fields_list:
                        missing_fields_list.append(idx)
            else:
                missing_counts += 1

        incomplete_mask = missing_counts >= 2
        status_series.loc[incomplete_mask] = "数据不完整"
        notes_series.loc[incomplete_mask] = "关键数据缺失，分析可能不准确。"

        if "rmsDbAbove18k" in df.columns:
            rms_18k = self._safe_fillna(df["rmsDbAbove18k"], 0)

            fake_mask = (rms_18k < self.thresholds.spectrum_fake_threshold) & (
                ~incomplete_mask
            )
            status_series.loc[fake_mask] = "可疑 (伪造)"
            notes_series.loc[fake_mask] = (
                "频谱在约 18kHz 处存在硬性截止 (高度疑似伪造/升频)。"
            )

            processed_mask = (
                (rms_18k < self.thresholds.spectrum_processed_threshold)
                & (rms_18k >= self.thresholds.spectrum_fake_threshold)
                & (~incomplete_mask)
                & (~fake_mask)
            )
            status_series.loc[processed_mask] = "疑似处理"
            notes_series.loc[processed_mask] = (
                "频谱在 18kHz 处能量较低，可能存在软性截止。"
            )

        if peak_field and peak_field in df.columns:
            peak_values = self._safe_fillna(
                df[peak_field], -144.0 if peak_field == "peakAmplitudeDb" else 0.0
            )

            if peak_field == "peakAmplitudeDb":
                clipping_mask = (
                    (peak_values >= self.thresholds.peak_clipping_db)
                    & (~incomplete_mask)
                    & (~status_series.str.contains("可疑", na=False))
                )
            else:
                clipping_mask = (
                    (peak_values >= self.thresholds.peak_clipping_linear)
                    & (~incomplete_mask)
                    & (~status_series.str.contains("可疑", na=False))
                )

            status_series.loc[clipping_mask] = "已削波"
            notes_series.loc[clipping_mask] = np.where(
                notes_series.loc[clipping_mask] != "",
                notes_series.loc[clipping_mask] + " | 存在严重数字削波风险",
                "存在严重数字削波风险",
            )

            if peak_field == "peakAmplitudeDb":
                notes_series.loc[clipping_mask] += " (峰值接近0dB)。"
            else:
                notes_series.loc[clipping_mask] += "。"

        if "lra" in df.columns:
            lra_values = self._safe_fillna(df["lra"], 0)
            lra_valid = (lra_values > 0) & (~incomplete_mask)

            severe_compression_mask = (
                (lra_values < self.thresholds.lra_poor_max)
                & lra_valid
                & (~status_series.str.contains("可疑", na=False))
            )
            status_series.loc[severe_compression_mask] = "严重压缩"
            for idx in df[severe_compression_mask].index:
                lra_val = df.loc[idx, "lra"]
                note = f"动态范围极低 (LRA: {lra_val:.1f} LU)，严重过度压缩。"
                if notes_series.loc[idx] != "":
                    notes_series.loc[idx] += f" | {note}"
                else:
                    notes_series.loc[idx] = note

            low_dynamic_mask = (
                (lra_values >= self.thresholds.lra_poor_max)
                & (lra_values < self.thresholds.lra_low_max)
                & lra_valid
                & (~status_series.str.contains("可疑|严重压缩|已削波", na=False))
            )
            status_series.loc[low_dynamic_mask] = "低动态"
            for idx in df[low_dynamic_mask].index:
                lra_val = df.loc[idx, "lra"]
                note = f"动态范围过低 (LRA: {lra_val:.1f} LU)，可能过度压缩。"
                if notes_series.loc[idx] != "":
                    notes_series.loc[idx] += f" | {note}"
                else:
                    notes_series.loc[idx] = note

            too_high_mask = (
                (lra_values > self.thresholds.lra_too_high)
                & lra_valid
                & (~status_series.str.contains("可疑|严重压缩|已削波|低动态", na=False))
            )
            for idx in df[too_high_mask].index:
                lra_val = df.loc[idx, "lra"]
                note = f"动态范围过高 (LRA: {lra_val:.1f} LU)，可能需要压缩处理。"
                if notes_series.loc[idx] != "":
                    notes_series.loc[idx] += f" | {note}"
                else:
                    notes_series.loc[idx] = note

        default_mask = notes_series == ""
        notes_series.loc[default_mask] = "未发现明显的硬性技术问题。"

        return status_series, notes_series

    def _format_analysis_errors(self, value: Union[str, List[str], None]) -> str:
        """规范化 analysisErrors 字段，输出稳定的字符串。"""
        if value is None:
            return ""
        if isinstance(value, list):
            return " | ".join([str(v) for v in value if str(v).strip() != ""])
        return str(value).strip()

    def _calculate_quality_score_vectorized(self, df: pd.DataFrame) -> pd.Series:
        """原始的质量评分函数 - 完全恢复原算法"""
        MAX_SCORE_INTEGRITY, MAX_SCORE_DYNAMICS, MAX_SCORE_SPECTRUM = 40, 30, 30

        integrity_scores = pd.Series([0.0] * len(df))
        dynamics_scores = pd.Series([0.0] * len(df))
        spectrum_scores = pd.Series([0.0] * len(df))

        critical_fields = ["rmsDbAbove18k", "lra"]
        peak_field = None
        if "peakAmplitudeDb" in df.columns:
            peak_field = "peakAmplitudeDb"
            critical_fields.append("peakAmplitudeDb")
        elif "peakAmplitude" in df.columns:
            peak_field = "peakAmplitude"
            critical_fields.append("peakAmplitude")

        completeness_penalty = pd.Series([0] * len(df))
        for field in critical_fields:
            if field in df.columns:
                completeness_penalty += (df[field].isna() | (df[field] == 0.0)).astype(
                    int
                ) * 10
            else:
                completeness_penalty += 10

        if "rmsDbAbove18k" in df.columns:
            rms_18k = self._safe_fillna(df["rmsDbAbove18k"], 0)
            valid_rms = rms_18k != 0

            excellent_mask = (
                rms_18k >= self.thresholds.spectrum_good_threshold
            ) & valid_rms
            integrity_scores.loc[excellent_mask] += 25

            good_mask = (
                (rms_18k >= self.thresholds.spectrum_processed_threshold)
                & (rms_18k < self.thresholds.spectrum_good_threshold)
                & valid_rms
            )
            integrity_scores.loc[good_mask] += self._map_to_score_vectorized(
                rms_18k.loc[good_mask],
                self.thresholds.spectrum_processed_threshold,
                self.thresholds.spectrum_good_threshold,
                15,
                25,
            )

            medium_mask = (
                (rms_18k >= self.thresholds.spectrum_fake_threshold)
                & (rms_18k < self.thresholds.spectrum_processed_threshold)
                & valid_rms
            )
            integrity_scores.loc[medium_mask] += self._map_to_score_vectorized(
                rms_18k.loc[medium_mask],
                self.thresholds.spectrum_fake_threshold,
                self.thresholds.spectrum_processed_threshold,
                5,
                15,
            )

        if peak_field and peak_field in df.columns:
            peak_values = self._safe_fillna(
                df[peak_field], -144.0 if peak_field == "peakAmplitudeDb" else 0.0
            )
            valid_peak = ~df[peak_field].isna()

            if peak_field == "peakAmplitudeDb":
                excellent_mask = (
                    peak_values <= self.thresholds.peak_good_db
                ) & valid_peak
                integrity_scores.loc[excellent_mask] += 15

                good_mask = (
                    (peak_values > self.thresholds.peak_good_db)
                    & (peak_values <= self.thresholds.peak_medium_db)
                    & valid_peak
                )
                integrity_scores.loc[good_mask] += self._map_to_score_vectorized(
                    peak_values.loc[good_mask],
                    self.thresholds.peak_good_db,
                    self.thresholds.peak_medium_db,
                    15,
                    10,
                )

                medium_mask = (
                    (peak_values > self.thresholds.peak_medium_db)
                    & (peak_values <= self.thresholds.peak_clipping_db)
                    & valid_peak
                )
                integrity_scores.loc[medium_mask] += self._map_to_score_vectorized(
                    peak_values.loc[medium_mask],
                    self.thresholds.peak_medium_db,
                    self.thresholds.peak_clipping_db,
                    10,
                    3,
                )
            else:
                excellent_mask = (peak_values <= 0.5) & valid_peak
                integrity_scores.loc[excellent_mask] += 15

                good_mask = (peak_values > 0.5) & (peak_values <= 0.8) & valid_peak
                integrity_scores.loc[good_mask] += self._map_to_score_vectorized(
                    peak_values.loc[good_mask], 0.5, 0.8, 15, 10
                )

                medium_mask = (peak_values > 0.8) & (peak_values <= 0.999) & valid_peak
                integrity_scores.loc[medium_mask] += self._map_to_score_vectorized(
                    peak_values.loc[medium_mask], 0.8, 0.999, 10, 3
                )

        if "lra" in df.columns:
            lra_values = self._safe_fillna(df["lra"], 0)
            valid_lra = lra_values > 0

            ideal_mask = (
                (lra_values >= self.thresholds.lra_excellent_min)
                & (lra_values <= self.thresholds.lra_excellent_max)
                & valid_lra
            )
            dynamics_scores.loc[ideal_mask] = 30

            low_acceptable_mask = (
                (lra_values >= self.thresholds.lra_low_max)
                & (lra_values < self.thresholds.lra_excellent_min)
                & valid_lra
            )
            dynamics_scores.loc[low_acceptable_mask] = self._map_to_score_vectorized(
                lra_values.loc[low_acceptable_mask],
                self.thresholds.lra_low_max,
                self.thresholds.lra_excellent_min,
                20,
                28,
            )

            high_mask = (
                (lra_values > self.thresholds.lra_excellent_max)
                & (lra_values <= self.thresholds.lra_acceptable_max)
                & valid_lra
            )
            dynamics_scores.loc[high_mask] = self._map_to_score_vectorized(
                lra_values.loc[high_mask],
                self.thresholds.lra_excellent_max,
                self.thresholds.lra_acceptable_max,
                28,
                22,
            )

            low_mask = (
                (lra_values >= self.thresholds.lra_poor_max)
                & (lra_values < self.thresholds.lra_low_max)
                & valid_lra
            )
            dynamics_scores.loc[low_mask] = self._map_to_score_vectorized(
                lra_values.loc[low_mask],
                self.thresholds.lra_poor_max,
                self.thresholds.lra_low_max,
                10,
                20,
            )

            very_low_mask = (lra_values < self.thresholds.lra_poor_max) & valid_lra
            dynamics_scores.loc[very_low_mask] = self._map_to_score_vectorized(
                lra_values.loc[very_low_mask], 0, self.thresholds.lra_poor_max, 0, 10
            )

            too_high_mask = (
                lra_values > self.thresholds.lra_acceptable_max
            ) & valid_lra
            dynamics_scores.loc[too_high_mask] = 18

        if "rmsDbAbove16k" in df.columns:
            rms_16k = self._safe_fillna(df["rmsDbAbove16k"], -90)
            spectrum_scores = self._map_to_score_vectorized(rms_16k, -90, -55, 0, 30)

        total_scores = (
            integrity_scores + dynamics_scores + spectrum_scores - completeness_penalty
        )

        if "状态" in df.columns:
            fake_mask = df["状态"] == "可疑 (伪造)"
            total_scores.loc[fake_mask] = np.minimum(total_scores.loc[fake_mask], 20)

            incomplete_mask = df["状态"] == "数据不完整"
            total_scores.loc[incomplete_mask] = np.minimum(
                total_scores.loc[incomplete_mask], 40
            )

        return np.maximum(0, total_scores.round()).astype(int)

    def analyze_dataframe(self, df: pd.DataFrame) -> pd.DataFrame:
        """分析完整的DataFrame"""
        if df.empty:
            logger.warning("输入DataFrame为空")
            return df

        self.stats["total_files"] = len(df)
        logger.info("-" * 40)
        logger.info(f"Python分析模块启动，共 {len(df)} 个文件待处理。")
        logger.info("-" * 40)

        start_time = time.time()

        with tqdm(
            total=3,
            desc="[ Python 端分析进度 ]",
            bar_format="{l_bar}{bar}| {n_fmt}/{total_fmt}",
        ) as pbar:
            pbar.set_postfix_str("Step 1: 分析状态与备注...")
            status_series, notes_series = self._analyze_row_vectorized(df)

            if "analysisErrors" in df.columns:
                analysis_error_text = df["analysisErrors"].apply(
                    self._format_analysis_errors
                )
                has_analysis_errors = analysis_error_text != ""
                status_series.loc[
                    has_analysis_errors & (status_series == "质量良好")
                ] = "分析异常"
                notes_series.loc[has_analysis_errors] = np.where(
                    notes_series.loc[has_analysis_errors] != "",
                    notes_series.loc[has_analysis_errors]
                    + " | 分析过程异常: "
                    + analysis_error_text.loc[has_analysis_errors],
                    "分析过程异常: " + analysis_error_text.loc[has_analysis_errors],
                )

            df["状态"] = status_series
            df["备注"] = notes_series
            time.sleep(0.1)
            pbar.update(1)

            pbar.set_postfix_str("Step 2: 计算综合质量分...")
            df["质量分"] = self._calculate_quality_score_vectorized(df)
            time.sleep(0.1)
            pbar.update(1)

            pbar.set_postfix_str("Step 3: 格式化与排序...")
            report_df = self.format_output_dataframe(df)
            time.sleep(0.1)
            pbar.update(1)

            pbar.set_postfix_str("分析完成!")

        self.stats["processing_time"] = time.time() - start_time
        self.stats["processed_files"] = len(df)

        logger.info(f"Python 端分析完成，耗时 {self.stats['processing_time']:.2f} 秒")
        logger.info("-" * 40)

        return report_df

    def format_output_dataframe(self, df: pd.DataFrame) -> pd.DataFrame:
        """格式化输出DataFrame"""
        peak_field = None
        if "peakAmplitudeDb" in df.columns:
            peak_field = "peakAmplitudeDb"
        elif "peakAmplitude" in df.columns:
            peak_field = "peakAmplitude"

        output_columns = ["质量分", "状态", "filePath", "备注", "lra"]
        if peak_field:
            output_columns.append(peak_field)

        additional_fields = [
            "rmsDbAbove16k",
            "rmsDbAbove18k",
            "rmsDbAbove20k",
            "overallRmsDb",
            "analysisErrors",
        ]
        for field in additional_fields:
            if field in df.columns:
                output_columns.append(field)

        final_columns = [col for col in output_columns if col in df.columns]
        result_df = df[final_columns].copy()
        result_df = result_df.sort_values(by="质量分", ascending=False)

        return result_df


def _safe_float(value: object, default: Optional[float] = None) -> Optional[float]:
    try:
        if value is None:
            return default
        return float(value)
    except (TypeError, ValueError):
        return default


def _analyze_without_pandas(records: List[Dict], min_score: int) -> List[Dict]:
    """
    无 pandas 环境下的降级分析流程。
    保持核心状态判断与错误透传能力，确保可生成可用报告。
    """
    thresholds = QualityThresholds()
    analyzed = []

    for row in records:
        lra = _safe_float(row.get("lra"))
        peak_db = _safe_float(row.get("peakAmplitudeDb"))
        rms_16k = _safe_float(row.get("rmsDbAbove16k"))
        rms_18k = _safe_float(row.get("rmsDbAbove18k"))

        status = "质量良好"
        notes = []

        missing_critical = 0
        for key in ("rmsDbAbove18k", "lra", "peakAmplitudeDb"):
            value = _safe_float(row.get(key))
            if value is None or value == 0.0:
                missing_critical += 1

        if missing_critical >= 2:
            status = "数据不完整"
            notes.append("关键数据缺失，分析可能不准确。")
        else:
            if rms_18k is not None:
                if rms_18k < thresholds.spectrum_fake_threshold:
                    status = "可疑 (伪造)"
                    notes.append("频谱在约 18kHz 处存在硬性截止 (高度疑似伪造/升频)。")
                elif rms_18k < thresholds.spectrum_processed_threshold:
                    status = "疑似处理"
                    notes.append("频谱在 18kHz 处能量较低，可能存在软性截止。")

            if peak_db is not None and peak_db >= thresholds.peak_clipping_db:
                if "可疑" not in status:
                    status = "已削波"
                notes.append("存在严重数字削波风险 (峰值接近0dB)。")

            if lra is not None and lra > 0:
                if lra < thresholds.lra_poor_max and "可疑" not in status:
                    status = "严重压缩"
                    notes.append(f"动态范围极低 (LRA: {lra:.1f} LU)，严重过度压缩。")
                elif (
                    lra < thresholds.lra_low_max
                    and "可疑" not in status
                    and status != "已削波"
                ):
                    status = "低动态"
                    notes.append(f"动态范围过低 (LRA: {lra:.1f} LU)，可能过度压缩。")
                elif lra > thresholds.lra_too_high:
                    notes.append(f"动态范围过高 (LRA: {lra:.1f} LU)，可能需要压缩处理。")

        analysis_errors_raw = row.get("analysisErrors", [])
        if isinstance(analysis_errors_raw, list):
            analysis_errors_text = " | ".join(
                [str(e).strip() for e in analysis_errors_raw if str(e).strip() != ""]
            )
        elif analysis_errors_raw:
            analysis_errors_text = str(analysis_errors_raw).strip()
        else:
            analysis_errors_text = ""

        if analysis_errors_text:
            if status == "质量良好":
                status = "分析异常"
            notes.append(f"分析过程异常: {analysis_errors_text}")

        if not notes:
            notes.append("未发现明显的硬性技术问题。")

        # 轻量评分：以 100 为基线，根据缺失与风险项扣分。
        score = 100 - missing_critical * 12
        if rms_18k is not None:
            if rms_18k < thresholds.spectrum_fake_threshold:
                score -= 50
            elif rms_18k < thresholds.spectrum_processed_threshold:
                score -= 25
        else:
            score -= 10

        if peak_db is None:
            score -= 8
        elif peak_db >= thresholds.peak_clipping_db:
            score -= 25
        elif peak_db > thresholds.peak_medium_db:
            score -= 12

        if lra is None or lra <= 0:
            score -= 10
        elif lra < thresholds.lra_poor_max:
            score -= 30
        elif lra < thresholds.lra_low_max:
            score -= 18
        elif lra > thresholds.lra_acceptable_max:
            score -= 10

        if status == "可疑 (伪造)":
            score = min(score, 20)
        if status == "数据不完整":
            score = min(score, 40)

        score = int(max(0, min(100, round(score))))

        out = dict(row)
        out["质量分"] = score
        out["状态"] = status
        out["备注"] = " | ".join(notes)
        analyzed.append(out)

    analyzed = [r for r in analyzed if int(r.get("质量分", 0)) >= min_score]
    analyzed.sort(key=lambda r: int(r.get("质量分", 0)), reverse=True)
    return analyzed


def _write_csv_without_pandas(records: List[Dict], output_path: str) -> None:
    if not records:
        # 写一个最小 CSV 表头，保证输出文件存在。
        with open(output_path, "w", newline="", encoding="utf-8-sig") as f:
            writer = csv.writer(f)
            writer.writerow(["质量分", "状态", "filePath", "备注"])
        return

    preferred_columns = [
        "质量分",
        "状态",
        "filePath",
        "备注",
        "lra",
        "peakAmplitudeDb",
        "rmsDbAbove16k",
        "rmsDbAbove18k",
        "rmsDbAbove20k",
        "overallRmsDb",
        "analysisErrors",
    ]
    dynamic_columns = []
    for col in preferred_columns:
        if any(col in r for r in records):
            dynamic_columns.append(col)
    for row in records:
        for col in row.keys():
            if col not in dynamic_columns:
                dynamic_columns.append(col)

    with open(output_path, "w", newline="", encoding="utf-8-sig") as f:
        writer = csv.DictWriter(f, fieldnames=dynamic_columns, extrasaction="ignore")
        writer.writeheader()
        for row in records:
            writer.writerow(row)


def _print_optional_stats_without_pandas(records: List[Dict], total_input: int) -> None:
    if not records:
        print("\n📈 详细统计:")
        print(" - 输出记录数: 0")
        return

    scores = [int(row.get("质量分", 0)) for row in records]
    avg_score = sum(scores) / len(scores)
    print("\n📈 详细统计:")
    print(f" - 输入记录数: {total_input}")
    print(f" - 输出记录数: {len(records)}")
    print(f" - 平均分: {avg_score:.1f}")
    print(f" - 最高分: {max(scores)}")
    print(f" - 最低分: {min(scores)}")


def _print_incomplete_without_pandas(records: List[Dict], limit: int = 10) -> None:
    incomplete = [row for row in records if str(row.get("状态", "")) == "数据不完整"]
    print(f"\n🧩 数据不完整文件: {len(incomplete)}")
    for idx, row in enumerate(incomplete[:limit], 1):
        filename = os.path.basename(str(row.get("filePath", "Unknown")))
        note = str(row.get("备注", ""))
        print(f" {idx}. {filename} -> {note}")


def main():
    """主执行函数"""
    parser = argparse.ArgumentParser(
        description=(
            f"分析由 audio_analyzer (Rust) 生成的 JSON 数据 (v{APP_VERSION} "
            "PyInstaller兼容版)。"
        )
    )

    parser.add_argument("input_json", help="输入的 analysis_data.json 文件路径。")
    parser.add_argument(
        "-o",
        "--output",
        default="audio_quality_report_v4.csv",
        help="输出的 CSV 报告文件名。",
    )
    parser.add_argument(
        "--min-score", type=int, default=0, help="只显示高于指定分数的文件 (默认: 0)。"
    )
    parser.add_argument(
        "--show-incomplete", action="store_true", help="显示数据不完整的文件详情。"
    )
    parser.add_argument("--show-stats", action="store_true", help="显示详细统计信息。")

    args = parser.parse_args()

    if not os.path.exists(args.input_json):
        print(f"错误: 输入文件 '{args.input_json}' 不存在。", file=sys.stderr)
        return 1

    try:
        if HAS_PANDAS:
            df = pd.read_json(args.input_json)
        else:
            with open(args.input_json, "r", encoding="utf-8") as f:
                df = json.load(f)
    except Exception as e:
        print(f"错误: 无法解析JSON文件: {e}", file=sys.stderr)
        return 1

    try:
        if HAS_PANDAS:
            if df.empty:
                print("JSON 文件为空，没有可分析的数据。")
                return 0

            analyzer = AudioQualityAnalyzer()
            report_df = analyzer.analyze_dataframe(df)

            if args.min_score > 0:
                original_count = len(report_df)
                report_df = report_df[report_df["质量分"] >= args.min_score]
                filtered_count = original_count - len(report_df)
                if filtered_count > 0:
                    print(f"已过滤掉 {filtered_count} 个低分文件 (< {args.min_score}分)")

            report_df.to_csv(args.output, index=False, encoding="utf-8-sig")
            print(f"\n✅ 完整的分析报告已保存到: {args.output}")
            if len(report_df) < len(df):
                filtered_count = len(df) - len(report_df)
                print(f" (已过滤掉 {filtered_count} 个低分文件)")

            print(f"\n--- 优化分析摘要 (v{APP_VERSION}) ---")
            status_counts = report_df["状态"].value_counts()
            print(f"\n📊 质量状态分布:")
            summary_total = max(len(report_df), 1)
            for status, count in status_counts.items():
                percentage = (count / summary_total) * 100
                print(f" - {status}: {count} 个文件 ({percentage:.1f}%)")

            print(f"\n🏆 质量排名前 5 的文件:")
            for i, (_, row) in enumerate(report_df.head(5).iterrows(), 1):
                filename = (
                    os.path.basename(row["filePath"]) if "filePath" in row else "Unknown"
                )
                print(f" {i}. [分数: {int(row['质量分'])}] {filename}")

            if args.show_stats:
                if len(report_df) > 0:
                    print("\n📈 详细统计:")
                    print(f" - 输入记录数: {len(df)}")
                    print(f" - 输出记录数: {len(report_df)}")
                    print(f" - 平均分: {report_df['质量分'].mean():.1f}")
                    print(f" - 最高分: {int(report_df['质量分'].max())}")
                    print(f" - 最低分: {int(report_df['质量分'].min())}")
                else:
                    print("\n📈 详细统计:")
                    print(f" - 输入记录数: {len(df)}")
                    print(" - 输出记录数: 0")
                print(f" - Python分析耗时: {analyzer.stats['processing_time']:.2f}s")

            if args.show_incomplete:
                incomplete_df = report_df[report_df["状态"] == "数据不完整"]
                print(f"\n🧩 数据不完整文件: {len(incomplete_df)}")
                for i, (_, row) in enumerate(incomplete_df.head(10).iterrows(), 1):
                    filename = (
                        os.path.basename(row["filePath"])
                        if "filePath" in row
                        else "Unknown"
                    )
                    note = row["备注"] if "备注" in row else ""
                    print(f" {i}. {filename} -> {note}")

            return 0

        if not isinstance(df, list):
            print("错误: 无 pandas 模式下，输入 JSON 必须是对象数组。", file=sys.stderr)
            return 1

        if len(df) == 0:
            print("JSON 文件为空，没有可分析的数据。")
            return 0

        analyzed_records = _analyze_without_pandas(df, args.min_score)
        _write_csv_without_pandas(analyzed_records, args.output)
        print(f"\n✅ 完整的分析报告已保存到: {args.output}")

        if len(analyzed_records) < len(df):
            filtered_count = len(df) - len(analyzed_records)
            print(f" (已过滤掉 {filtered_count} 个低分文件)")

        print("\n--- 优化分析摘要 (fallback) ---")
        status_counts: Dict[str, int] = {}
        for row in analyzed_records:
            status = str(row.get("状态", "未知状态"))
            status_counts[status] = status_counts.get(status, 0) + 1

        print("\n📊 质量状态分布:")
        summary_total = max(len(analyzed_records), 1)
        for status, count in status_counts.items():
            percentage = (count / summary_total) * 100
            print(f" - {status}: {count} 个文件 ({percentage:.1f}%)")

        print("\n🏆 质量排名前 5 的文件:")
        for i, row in enumerate(analyzed_records[:5], 1):
            filename = os.path.basename(str(row.get("filePath", "Unknown")))
            score = int(row.get("质量分", 0))
            print(f" {i}. [分数: {score}] {filename}")

        if args.show_stats:
            _print_optional_stats_without_pandas(analyzed_records, len(df))

        if args.show_incomplete:
            _print_incomplete_without_pandas(analyzed_records)

        return 0

    except Exception as e:
        print(f"分析过程中出错: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    if getattr(sys, "frozen", False):
        multiprocessing.freeze_support()

    sys.exit(main())
