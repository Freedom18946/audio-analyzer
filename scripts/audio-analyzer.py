#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
音频质量分析器 v4.1 (带进度条优化版) - PyInstaller兼容版
保持原始评分算法完整性
"""

import sys
import os

# PyInstaller兼容性修复 - 在其他导入之前
if getattr(sys, 'frozen', False):
    import multiprocessing
    multiprocessing.freeze_support()

    if sys.platform == 'darwin':
        bundle_dir = getattr(sys, '_MEIPASS', os.path.dirname(os.path.abspath(__file__)))
        os.environ['DYLD_LIBRARY_PATH'] = bundle_dir + ':' + os.environ.get('DYLD_LIBRARY_PATH', '')

try:
    import multiprocessing
    if sys.platform in ['win32', 'darwin']:
        if __name__ == "__main__":
            multiprocessing.set_start_method('spawn', force=True)
except (ImportError, RuntimeError):
    pass

import pandas as pd
import numpy as np
import argparse
import json
import time
from pathlib import Path
from typing import Dict, List, Tuple, Optional
from dataclasses import dataclass
import logging
import warnings

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
                print(f"{desc} 进度: {progress:.1f}%")

        def set_postfix_str(self, s):
            print(f"  {s}")

        def __enter__(self):
            return self

        def __exit__(self, *args):
            print(f"{self.desc} 完成!")

warnings.filterwarnings('ignore', category=pd.errors.PerformanceWarning)
warnings.filterwarnings('ignore', category=UserWarning)

logging.basicConfig(level=logging.INFO, format='%(message)s')
logger = logging.getLogger(__name__)

@dataclass
class QualityThresholds:
    """质量评分阈值配置（与原版保持一致）"""
    spectrum_fake_threshold: float = -85.0
    spectrum_processed_threshold: float = -80.0
    spectrum_good_threshold: float = -70.0
    lra_poor_max: float = 3.0
    lra_low_max: float = 6.0
    lra_excellent_min: float = 8.0
    lra_excellent_max: float = 12.0
    lra_acceptable_max: float = 15.0
    lra_too_high: float = 20.0
    peak_clipping_db: float = -0.1
    peak_clipping_linear: float = 0.999
    peak_good_db: float = -6.0
    peak_medium_db: float = -3.0

class AudioQualityAnalyzer:
    """高性能音频质量分析器（PyInstaller兼容版 - 保持原始评分算法）"""

    def __init__(self):
        self.thresholds = QualityThresholds()
        self.stats = {
            'total_files': 0,
            'processed_files': 0,
            'processing_time': 0.0
        }

    def _safe_fillna(self, series, value=0):
        """安全的fillna操作"""
        try:
            return series.fillna(value)
        except Exception:
            return series.replace([np.nan, None], value)

    def _map_to_score_vectorized(self, values: pd.Series, in_min: float, in_max: float, out_min: float = 0, out_max: float = 1) -> pd.Series:
        """原始的分数映射函数 - 保持不变"""
        values = self._safe_fillna(values, 0)
        values = np.clip(values, in_min, in_max)
        if in_max == in_min:
            return pd.Series([out_min] * len(values))
        return out_min + (values - in_min) * (out_max - out_min) / (in_max - in_min)

    def _analyze_row_vectorized(self, df: pd.DataFrame) -> Tuple[pd.Series, pd.Series]:
        """原始的状态分析函数 - 保持完全不变"""
        status_series = pd.Series(['质量良好'] * len(df))
        notes_series = pd.Series([''] * len(df))

        critical_fields = ['rmsDbAbove18k', 'lra']
        peak_field = None
        if 'peakAmplitudeDb' in df.columns:
            peak_field = 'peakAmplitudeDb'
            critical_fields.append('peakAmplitudeDb')
        elif 'peakAmplitude' in df.columns:
            peak_field = 'peakAmplitude'
            critical_fields.append('peakAmplitude')

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
        status_series.loc[incomplete_mask] = '数据不完整'
        notes_series.loc[incomplete_mask] = '关键数据缺失，分析可能不准确。'

        if 'rmsDbAbove18k' in df.columns:
            rms_18k = self._safe_fillna(df['rmsDbAbove18k'], 0)

            fake_mask = (rms_18k < self.thresholds.spectrum_fake_threshold) & (~incomplete_mask)
            status_series.loc[fake_mask] = '可疑 (伪造)'
            notes_series.loc[fake_mask] = '频谱在约 18kHz 处存在硬性截止 (高度疑似伪造/升频)。'

            processed_mask = (rms_18k < self.thresholds.spectrum_processed_threshold) & (rms_18k >= self.thresholds.spectrum_fake_threshold) & (~incomplete_mask) & (~fake_mask)
            status_series.loc[processed_mask] = '疑似处理'
            notes_series.loc[processed_mask] = '频谱在 18kHz 处能量较低，可能存在软性截止。'

        if peak_field and peak_field in df.columns:
            peak_values = self._safe_fillna(df[peak_field], -144.0 if peak_field == 'peakAmplitudeDb' else 0.0)

            if peak_field == 'peakAmplitudeDb':
                clipping_mask = (peak_values >= self.thresholds.peak_clipping_db) & (~incomplete_mask) & (~status_series.str.contains('可疑', na=False))
            else:
                clipping_mask = (peak_values >= self.thresholds.peak_clipping_linear) & (~incomplete_mask) & (~status_series.str.contains('可疑', na=False))

            status_series.loc[clipping_mask] = '已削波'
            notes_series.loc[clipping_mask] = np.where(notes_series.loc[clipping_mask] != '', notes_series.loc[clipping_mask] + ' | 存在严重数字削波风险', '存在严重数字削波风险')

            if peak_field == 'peakAmplitudeDb':
                notes_series.loc[clipping_mask] += ' (峰值接近0dB)。'
            else:
                notes_series.loc[clipping_mask] += '。'

        if 'lra' in df.columns:
            lra_values = self._safe_fillna(df['lra'], 0)
            lra_valid = (lra_values > 0) & (~incomplete_mask)

            severe_compression_mask = (lra_values < self.thresholds.lra_poor_max) & lra_valid & (~status_series.str.contains('可疑', na=False))
            status_series.loc[severe_compression_mask] = '严重压缩'
            for idx in df[severe_compression_mask].index:
                lra_val = df.loc[idx, 'lra']
                note = f'动态范围极低 (LRA: {lra_val:.1f} LU)，严重过度压缩。'
                if notes_series.loc[idx] != '':
                    notes_series.loc[idx] += f' | {note}'
                else:
                    notes_series.loc[idx] = note

            low_dynamic_mask = (lra_values >= self.thresholds.lra_poor_max) & (lra_values < self.thresholds.lra_low_max) & lra_valid & (~status_series.str.contains('可疑|严重压缩|已削波', na=False))
            status_series.loc[low_dynamic_mask] = '低动态'
            for idx in df[low_dynamic_mask].index:
                lra_val = df.loc[idx, 'lra']
                note = f'动态范围过低 (LRA: {lra_val:.1f} LU)，可能过度压缩。'
                if notes_series.loc[idx] != '':
                    notes_series.loc[idx] += f' | {note}'
                else:
                    notes_series.loc[idx] = note

            too_high_mask = (lra_values > self.thresholds.lra_too_high) & lra_valid & (~status_series.str.contains('可疑|严重压缩|已削波|低动态', na=False))
            for idx in df[too_high_mask].index:
                lra_val = df.loc[idx, 'lra']
                note = f'动态范围过高 (LRA: {lra_val:.1f} LU)，可能需要压缩处理。'
                if notes_series.loc[idx] != '':
                    notes_series.loc[idx] += f' | {note}'
                else:
                    notes_series.loc[idx] = note

        default_mask = notes_series == ''
        notes_series.loc[default_mask] = '未发现明显的硬性技术问题。'

        return status_series, notes_series

    def _calculate_quality_score_vectorized(self, df: pd.DataFrame) -> pd.Series:
        """原始的质量评分函数 - 完全恢复原算法"""
        MAX_SCORE_INTEGRITY, MAX_SCORE_DYNAMICS, MAX_SCORE_SPECTRUM = 40, 30, 30

        integrity_scores = pd.Series([0.0] * len(df))
        dynamics_scores = pd.Series([0.0] * len(df))
        spectrum_scores = pd.Series([0.0] * len(df))

        critical_fields = ['rmsDbAbove18k', 'lra']
        peak_field = None
        if 'peakAmplitudeDb' in df.columns:
            peak_field = 'peakAmplitudeDb'
            critical_fields.append('peakAmplitudeDb')
        elif 'peakAmplitude' in df.columns:
            peak_field = 'peakAmplitude'
            critical_fields.append('peakAmplitude')

        completeness_penalty = pd.Series([0] * len(df))
        for field in critical_fields:
            if field in df.columns:
                completeness_penalty += (df[field].isna() | (df[field] == 0.0)).astype(int) * 10
            else:
                completeness_penalty += 10

        if 'rmsDbAbove18k' in df.columns:
            rms_18k = self._safe_fillna(df['rmsDbAbove18k'], 0)
            valid_rms = rms_18k != 0

            excellent_mask = (rms_18k >= self.thresholds.spectrum_good_threshold) & valid_rms
            integrity_scores.loc[excellent_mask] += 25

            good_mask = (rms_18k >= self.thresholds.spectrum_processed_threshold) & (rms_18k < self.thresholds.spectrum_good_threshold) & valid_rms
            integrity_scores.loc[good_mask] += self._map_to_score_vectorized(rms_18k.loc[good_mask], self.thresholds.spectrum_processed_threshold, self.thresholds.spectrum_good_threshold, 15, 25)

            medium_mask = (rms_18k >= self.thresholds.spectrum_fake_threshold) & (rms_18k < self.thresholds.spectrum_processed_threshold) & valid_rms
            integrity_scores.loc[medium_mask] += self._map_to_score_vectorized(rms_18k.loc[medium_mask], self.thresholds.spectrum_fake_threshold, self.thresholds.spectrum_processed_threshold, 5, 15)

        if peak_field and peak_field in df.columns:
            peak_values = self._safe_fillna(df[peak_field], -144.0 if peak_field == 'peakAmplitudeDb' else 0.0)
            valid_peak = ~df[peak_field].isna()

            if peak_field == 'peakAmplitudeDb':
                excellent_mask = (peak_values <= self.thresholds.peak_good_db) & valid_peak
                integrity_scores.loc[excellent_mask] += 15

                good_mask = (peak_values > self.thresholds.peak_good_db) & (peak_values <= self.thresholds.peak_medium_db) & valid_peak
                integrity_scores.loc[good_mask] += self._map_to_score_vectorized(peak_values.loc[good_mask], self.thresholds.peak_good_db, self.thresholds.peak_medium_db, 15, 10)

                medium_mask = (peak_values > self.thresholds.peak_medium_db) & (peak_values <= self.thresholds.peak_clipping_db) & valid_peak
                integrity_scores.loc[medium_mask] += self._map_to_score_vectorized(peak_values.loc[medium_mask], self.thresholds.peak_medium_db, self.thresholds.peak_clipping_db, 10, 3)
            else:
                excellent_mask = (peak_values <= 0.5) & valid_peak
                integrity_scores.loc[excellent_mask] += 15

                good_mask = (peak_values > 0.5) & (peak_values <= 0.8) & valid_peak
                integrity_scores.loc[good_mask] += self._map_to_score_vectorized(peak_values.loc[good_mask], 0.5, 0.8, 15, 10)

                medium_mask = (peak_values > 0.8) & (peak_values <= 0.999) & valid_peak
                integrity_scores.loc[medium_mask] += self._map_to_score_vectorized(peak_values.loc[medium_mask], 0.8, 0.999, 10, 3)

        if 'lra' in df.columns:
            lra_values = self._safe_fillna(df['lra'], 0)
            valid_lra = lra_values > 0

            ideal_mask = (lra_values >= self.thresholds.lra_excellent_min) & (lra_values <= self.thresholds.lra_excellent_max) & valid_lra
            dynamics_scores.loc[ideal_mask] = 30

            low_acceptable_mask = (lra_values >= self.thresholds.lra_low_max) & (lra_values < self.thresholds.lra_excellent_min) & valid_lra
            dynamics_scores.loc[low_acceptable_mask] = self._map_to_score_vectorized(lra_values.loc[low_acceptable_mask], self.thresholds.lra_low_max, self.thresholds.lra_excellent_min, 20, 28)

            high_mask = (lra_values > self.thresholds.lra_excellent_max) & (lra_values <= self.thresholds.lra_acceptable_max) & valid_lra
            dynamics_scores.loc[high_mask] = self._map_to_score_vectorized(lra_values.loc[high_mask], self.thresholds.lra_excellent_max, self.thresholds.lra_acceptable_max, 28, 22)

            low_mask = (lra_values >= self.thresholds.lra_poor_max) & (lra_values < self.thresholds.lra_low_max) & valid_lra
            dynamics_scores.loc[low_mask] = self._map_to_score_vectorized(lra_values.loc[low_mask], self.thresholds.lra_poor_max, self.thresholds.lra_low_max, 10, 20)

            very_low_mask = (lra_values < self.thresholds.lra_poor_max) & valid_lra
            dynamics_scores.loc[very_low_mask] = self._map_to_score_vectorized(lra_values.loc[very_low_mask], 0, self.thresholds.lra_poor_max, 0, 10)

            too_high_mask = (lra_values > self.thresholds.lra_acceptable_max) & valid_lra
            dynamics_scores.loc[too_high_mask] = 18

        if 'rmsDbAbove16k' in df.columns:
            rms_16k = self._safe_fillna(df['rmsDbAbove16k'], -90)
            spectrum_scores = self._map_to_score_vectorized(rms_16k, -90, -55, 0, 30)

        total_scores = integrity_scores + dynamics_scores + spectrum_scores - completeness_penalty

        if '状态' in df.columns:
            fake_mask = df['状态'] == '可疑 (伪造)'
            total_scores.loc[fake_mask] = np.minimum(total_scores.loc[fake_mask], 20)

            incomplete_mask = df['状态'] == '数据不完整'
            total_scores.loc[incomplete_mask] = np.minimum(total_scores.loc[incomplete_mask], 40)

        return np.maximum(0, total_scores.round()).astype(int)

    def analyze_dataframe(self, df: pd.DataFrame) -> pd.DataFrame:
        """分析完整的DataFrame"""
        if df.empty:
            logger.warning("输入DataFrame为空")
            return df

        self.stats['total_files'] = len(df)
        logger.info("-" * 40)
        logger.info(f"Python分析模块启动，共 {len(df)} 个文件待处理。")
        logger.info("-" * 40)

        start_time = time.time()

        with tqdm(total=3, desc="[ Python 端分析进度 ]", bar_format="{l_bar}{bar}| {n_fmt}/{total_fmt}") as pbar:
            pbar.set_postfix_str("Step 1: 分析状态与备注...")
            status_series, notes_series = self._analyze_row_vectorized(df)
            df['状态'] = status_series
            df['备注'] = notes_series
            time.sleep(0.1)
            pbar.update(1)

            pbar.set_postfix_str("Step 2: 计算综合质量分...")
            df['质量分'] = self._calculate_quality_score_vectorized(df)
            time.sleep(0.1)
            pbar.update(1)

            pbar.set_postfix_str("Step 3: 格式化与排序...")
            report_df = self.format_output_dataframe(df)
            time.sleep(0.1)
            pbar.update(1)

            pbar.set_postfix_str("分析完成!")

        self.stats['processing_time'] = time.time() - start_time
        self.stats['processed_files'] = len(df)

        logger.info(f"Python 端分析完成，耗时 {self.stats['processing_time']:.2f} 秒")
        logger.info("-" * 40)

        return report_df

    def format_output_dataframe(self, df: pd.DataFrame) -> pd.DataFrame:
        """格式化输出DataFrame"""
        peak_field = None
        if 'peakAmplitudeDb' in df.columns:
            peak_field = 'peakAmplitudeDb'
        elif 'peakAmplitude' in df.columns:
            peak_field = 'peakAmplitude'

        output_columns = ['质量分', '状态', 'filePath', '备注', 'lra']
        if peak_field:
            output_columns.append(peak_field)

        additional_fields = ['rmsDbAbove16k', 'rmsDbAbove18k', 'rmsDbAbove20k', 'overallRmsDb']
        for field in additional_fields:
            if field in df.columns:
                output_columns.append(field)

        final_columns = [col for col in output_columns if col in df.columns]
        result_df = df[final_columns].copy()
        result_df = result_df.sort_values(by='质量分', ascending=False)

        return result_df

def main():
    """主执行函数"""
    parser = argparse.ArgumentParser(
        description="分析由 audio_analyzer (Rust) 生成的 JSON 数据 (v4.1 PyInstaller兼容版)。"
    )

    parser.add_argument("input_json", help="输入的 analysis_data.json 文件路径。")
    parser.add_argument("-o", "--output", default="audio_quality_report_v4.csv",
                       help="输出的 CSV 报告文件名。")
    parser.add_argument("--min-score", type=int, default=0,
                       help="只显示高于指定分数的文件 (默认: 0)。")
    parser.add_argument("--show-incomplete", action="store_true",
                       help="显示数据不完整的文件详情。")
    parser.add_argument("--show-stats", action="store_true",
                       help="显示详细统计信息。")

    args = parser.parse_args()

    if not os.path.exists(args.input_json):
        print(f"错误: 输入文件 '{args.input_json}' 不存在。", file=sys.stderr)
        return 1

    try:
        df = pd.read_json(args.input_json)
    except Exception as e:
        print(f"错误: 无法解析JSON文件: {e}", file=sys.stderr)
        return 1

    if df.empty:
        print("JSON 文件为空，没有可分析的数据。")
        return 0

    try:
        analyzer = AudioQualityAnalyzer()
        report_df = analyzer.analyze_dataframe(df)

        if args.min_score > 0:
            original_count = len(report_df)
            report_df = report_df[report_df['质量分'] >= args.min_score]
            filtered_count = original_count - len(report_df)
            if filtered_count > 0:
                print(f"已过滤掉 {filtered_count} 个低分文件 (< {args.min_score}分)")

        report_df.to_csv(args.output, index=False, encoding='utf-8-sig')
        print(f"\n✅ 完整的分析报告已保存到: {args.output}")
        if len(report_df) < len(df):
            filtered_count = len(df) - len(report_df)
            print(f" (已过滤掉 {filtered_count} 个低分文件)")

        print(f"\n--- 优化分析摘要 (v4.1) ---")
        status_counts = report_df['状态'].value_counts()
        print(f"\n📊 质量状态分布:")
        for status, count in status_counts.items():
            percentage = (count / len(df)) * 100
            print(f" - {status}: {count} 个文件 ({percentage:.1f}%)")

        print(f"\n🏆 质量排名前 5 的文件:")
        for i, (_, row) in enumerate(report_df.head(5).iterrows(), 1):
            filename = os.path.basename(row['filePath']) if 'filePath' in row else 'Unknown'
            print(f" {i}. [分数: {int(row['质量分'])}] {filename}")

        return 0

    except Exception as e:
        print(f"分析过程中出错: {e}", file=sys.stderr)
        return 1

if __name__ == "__main__":
    if getattr(sys, 'frozen', False):
        multiprocessing.freeze_support()

    sys.exit(main())
