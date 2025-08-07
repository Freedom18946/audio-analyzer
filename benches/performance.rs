//! # 性能基准测试
//!
//! 测试音频分析器的各种性能指标

use audio_analyzer_ultimate::{
    utils::{fs_utils, Timer},
    AnalyzerConfig, AudioAnalyzer,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::path::PathBuf;
use tempfile::TempDir;

/// 创建测试用的音频文件（模拟）
fn create_test_audio_files(count: usize) -> (TempDir, Vec<PathBuf>) {
    let temp_dir = TempDir::new().unwrap();
    let mut files = Vec::new();

    for i in 0..count {
        let file_path = temp_dir.path().join(format!("test_{i}.wav"));
        // 创建一个小的测试文件（实际应用中会是真实的音频数据）
        std::fs::write(&file_path, b"fake audio data").unwrap();
        files.push(file_path);
    }

    (temp_dir, files)
}

/// 基准测试：配置创建和验证
fn bench_config_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_operations");

    group.bench_function("create_default_config", |b| {
        b.iter(|| {
            let config = black_box(AnalyzerConfig::default());
            black_box(config)
        })
    });

    group.bench_function("validate_config", |b| {
        let config = AnalyzerConfig::default();
        b.iter(|| {
            let result = black_box(config.validate());
            black_box(result)
        })
    });

    group.bench_function("config_serialization", |b| {
        let config = AnalyzerConfig::default();
        let temp_file = tempfile::NamedTempFile::new().unwrap();

        b.iter(|| {
            let result = black_box(config.save_to_file(temp_file.path()));
            black_box(result)
        })
    });

    group.finish();
}

/// 基准测试：文件系统操作
fn bench_filesystem_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("filesystem_operations");

    // 测试不同数量的文件扫描
    for file_count in [10, 50, 100, 500].iter() {
        let (_temp_dir, _files) = create_test_audio_files(*file_count);
        let extensions = vec!["wav".to_string(), "mp3".to_string()];

        group.bench_with_input(
            BenchmarkId::new("scan_audio_files", file_count),
            file_count,
            |b, _| {
                b.iter(|| {
                    let result =
                        black_box(fs_utils::scan_audio_files(_temp_dir.path(), &extensions));
                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

/// 基准测试：字符串处理操作
fn bench_string_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_operations");

    use audio_analyzer_ultimate::utils::string_utils;

    group.bench_function("format_file_size", |b| {
        b.iter(|| {
            let result = black_box(string_utils::format_file_size(1048576));
            black_box(result)
        })
    });

    group.bench_function("format_duration", |b| {
        let duration = std::time::Duration::from_secs(3661);
        b.iter(|| {
            let result = black_box(string_utils::format_duration(duration));
            black_box(result)
        })
    });

    group.bench_function("truncate_string", |b| {
        let long_string = "a".repeat(1000);
        b.iter(|| {
            let result = black_box(string_utils::truncate_string(&long_string, 50));
            black_box(result)
        })
    });

    group.finish();
}

/// 基准测试：分析器初始化
fn bench_analyzer_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("analyzer_initialization");

    group.bench_function("create_analyzer", |b| {
        let config = AnalyzerConfig::default();
        b.iter(|| {
            let result = black_box(AudioAnalyzer::new(config.clone()));
            black_box(result)
        })
    });

    group.bench_function("initialize_dependencies", |b| {
        b.iter(|| {
            let config = AnalyzerConfig::default();
            let mut analyzer = AudioAnalyzer::new(config).unwrap();
            let result = black_box(analyzer.initialize_dependencies());
            black_box(result)
        })
    });

    group.finish();
}

/// 基准测试：内存使用模式
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");

    // 测试大量AudioMetrics对象的创建
    group.bench_function("create_many_metrics", |b| {
        b.iter(|| {
            let metrics: Vec<_> = (0..1000)
                .map(|i| {
                    black_box(audio_analyzer_ultimate::AudioMetrics::new(
                        format!("file_{i}.wav"),
                        1024 * i as u64,
                    ))
                })
                .collect();
            black_box(metrics)
        })
    });

    // 测试JSON序列化性能
    group.bench_function("serialize_metrics", |b| {
        let metrics: Vec<_> = (0..100)
            .map(|i| {
                audio_analyzer_ultimate::AudioMetrics::new(format!("file_{i}.wav"), 1024 * i as u64)
            })
            .collect();

        b.iter(|| {
            let result = black_box(serde_json::to_string(&metrics));
            black_box(result)
        })
    });

    group.finish();
}

/// 基准测试：并发性能
fn bench_concurrency(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrency");

    // 测试不同线程数的性能
    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("parallel_processing", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    use rayon::prelude::*;

                    // 设置线程池大小
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(thread_count)
                        .build()
                        .unwrap();

                    pool.install(|| {
                        let data: Vec<i32> = (0..1000).collect();
                        let result: Vec<_> = data
                            .par_iter()
                            .map(|&x| {
                                // 模拟一些计算工作
                                black_box(x * x + x * 2 + 1)
                            })
                            .collect();
                        black_box(result)
                    })
                })
            },
        );
    }

    group.finish();
}

/// 基准测试：Timer性能
fn bench_timer_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("timer_operations");

    group.bench_function("timer_creation", |b| {
        b.iter(|| {
            let timer = black_box(Timer::new("test"));
            black_box(timer)
        })
    });

    group.bench_function("timer_elapsed", |b| {
        let timer = Timer::new("test");
        b.iter(|| {
            let elapsed = black_box(timer.elapsed());
            black_box(elapsed)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_config_operations,
    bench_filesystem_operations,
    bench_string_operations,
    bench_analyzer_initialization,
    bench_memory_patterns,
    bench_concurrency,
    bench_timer_operations
);

criterion_main!(benches);
