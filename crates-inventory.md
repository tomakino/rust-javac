# crates 目录文件清单

共 536 个文件, 合计 10.7 MB, 文本 283592 行 (生成时间 2026-10-27 17:09:24). 二进制文件行数记 "-".

## javac-charset — 8 个文件 / 95.9 KB / 2534 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-charset\Cargo.toml` | 155 B | 8 |
| `javac-charset\src\charset.rs` | 24.7 KB | 759 |
| `javac-charset\src\coder.rs` | 21.7 KB | 636 |
| `javac-charset\src\error.rs` | 2.7 KB | 73 |
| `javac-charset\src\lib.rs` | 2.2 KB | 47 |
| `javac-charset\src\nio.rs` | 11.1 KB | 384 |
| `javac-charset\src\oracle_tests.rs` | 24.7 KB | 416 |
| `javac-charset\src\stream_tests.rs` | 8.6 KB | 211 |

## javac-classpath — 2 个文件 / 17.2 KB / 449 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-classpath\Cargo.toml` | 211 B | 10 |
| `javac-classpath\src\lib.rs` | 17.0 KB | 439 |

## javac-core — 214 个文件 / 8.2 MB / 212372 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-core\Cargo.toml` | 553 B | 21 |
| `javac-core\src\api\diagnostic_formatter.rs` | 8.2 KB | 221 |
| `javac-core\src\api\formattable.rs` | 3.7 KB | 114 |
| `javac-core\src\api\messages.rs` | 3.3 KB | 95 |
| `javac-core\src\api\mod.rs` | 98 B | 5 |
| `javac-core\src\code\attribute.rs` | 29.7 KB | 777 |
| `javac-core\src\code\bound_kind.rs` | 1.2 KB | 48 |
| `javac-core\src\code\class_finder.rs` | 64.7 KB | 1660 |
| `javac-core\src\code\deferred_completion_failure_handler.rs` | 11.8 KB | 372 |
| `javac-core\src\code\deferred_lint_handler.rs` | 4.5 KB | 131 |
| `javac-core\src\code\dependencies.rs` | 2.2 KB | 67 |
| `javac-core\src\code\directive.rs` | 10.5 KB | 310 |
| `javac-core\src\code\flags.rs` | 27.0 KB | 757 |
| `javac-core\src\code\kinds.rs` | 23.5 KB | 720 |
| `javac-core\src\code\lint.rs` | 28.2 KB | 715 |
| `javac-core\src\code\mod.rs` | 533 B | 26 |
| `javac-core\src\code\module_finder.rs` | 40.6 KB | 1118 |
| `javac-core\src\code\preview.rs` | 28.5 KB | 672 |
| `javac-core\src\code\scope.rs` | 88.0 KB | 2285 |
| `javac-core\src\code\source.rs` | 33.7 KB | 835 |
| `javac-core\src\code\symbol_metadata.rs` | 13.5 KB | 365 |
| `javac-core\src\code\symbol.rs` | 142.5 KB | 3904 |
| `javac-core\src\code\symtab.rs` | 88.8 KB | 2225 |
| `javac-core\src\code\target_type.rs` | 8.3 KB | 202 |
| `javac-core\src\code\type_.rs` | 71.5 KB | 1903 |
| `javac-core\src\code\type_annotation_position.rs` | 17.8 KB | 368 |
| `javac-core\src\code\type_annotations.rs` | 96.7 KB | 2227 |
| `javac-core\src\code\type_metadata.rs` | 5.8 KB | 181 |
| `javac-core\src\code\type_tag.rs` | 9.6 KB | 271 |
| `javac-core\src\code\types.rs` | 431.1 KB | 11055 |
| `javac-core\src\comp\analyzer.rs` | 47.5 KB | 1208 |
| `javac-core\src\comp\annotate.rs` | 108.3 KB | 2566 |
| `javac-core\src\comp\argument_attr.rs` | 62.5 KB | 1555 |
| `javac-core\src\comp\attr_context.rs` | 5.4 KB | 146 |
| `javac-core\src\comp\attr_recover.rs` | 36.0 KB | 894 |
| `javac-core\src\comp\attr.rs` | 699.9 KB | 16758 |
| `javac-core\src\comp\check.rs` | 412.2 KB | 10093 |
| `javac-core\src\comp\compile_states.rs` | 6.0 KB | 202 |
| `javac-core\src\comp\const_fold.rs` | 21.2 KB | 523 |
| `javac-core\src\comp\deferred_attr.rs` | 130.7 KB | 3499 |
| `javac-core\src\comp\enter.rs` | 73.0 KB | 1841 |
| `javac-core\src\comp\env.rs` | 6.5 KB | 202 |
| `javac-core\src\comp\flow.rs` | 229.0 KB | 5865 |
| `javac-core\src\comp\infer.rs` | 165.8 KB | 4521 |
| `javac-core\src\comp\lower.rs` | 365.0 KB | 8774 |
| `javac-core\src\comp\match_bindings.rs` | 18.5 KB | 506 |
| `javac-core\src\comp\member_enter.rs` | 47.1 KB | 1283 |
| `javac-core\src\comp\mod.rs` | 500 B | 26 |
| `javac-core\src\comp\modules.rs` | 142.2 KB | 3483 |
| `javac-core\src\comp\operators.rs` | 46.3 KB | 1150 |
| `javac-core\src\comp\resolve.rs` | 369.7 KB | 9462 |
| `javac-core\src\comp\todo.rs` | 7.6 KB | 243 |
| `javac-core\src\comp\trans_types.rs` | 97.7 KB | 2329 |
| `javac-core\src\comp\type_enter.rs` | 149.9 KB | 4075 |
| `javac-core\src\comp\type_envs.rs` | 1.6 KB | 60 |
| `javac-core\src\compiler_context.rs` | 47.2 KB | 1259 |
| `javac-core\src\file\base_file_manager_api.rs` | 3.2 KB | 79 |
| `javac-core\src\file\base_file_manager.rs` | 42.9 KB | 1088 |
| `javac-core\src\file\cache_fs_info.rs` | 11.6 KB | 308 |
| `javac-core\src\file\fs_info.rs` | 22.4 KB | 619 |
| `javac-core\src\file\javac_file_manager.rs` | 77.0 KB | 1988 |
| `javac-core\src\file\jrt_index.rs` | 20.7 KB | 530 |
| `javac-core\src\file\locations.rs` | 94.0 KB | 2470 |
| `javac-core\src\file\mod.rs` | 1.1 KB | 26 |
| `javac-core\src\file\path_file_object.rs` | 43.1 KB | 1140 |
| `javac-core\src\file\relative_path.rs` | 12.3 KB | 364 |
| `javac-core\src\ids.rs` | 3.6 KB | 104 |
| `javac-core\src\jvm\byte_codes.rs` | 13.4 KB | 390 |
| `javac-core\src\jvm\class_file.rs` | 6.4 KB | 213 |
| `javac-core\src\jvm\class_reader.rs` | 249.6 KB | 5819 |
| `javac-core\src\jvm\class_writer.rs` | 88.2 KB | 2265 |
| `javac-core\src\jvm\code.rs` | 97.4 KB | 3020 |
| `javac-core\src\jvm\codegen.rs` | 178.3 KB | 4367 |
| `javac-core\src\jvm\cr_table.rs` | 32.8 KB | 1022 |
| `javac-core\src\jvm\crt_flags.rs` | 1.0 KB | 35 |
| `javac-core\src\jvm\items.rs` | 40.8 KB | 1123 |
| `javac-core\src\jvm\jni_writer.rs` | 31.9 KB | 899 |
| `javac-core\src\jvm\mod.rs` | 433 B | 21 |
| `javac-core\src\jvm\module_name_reader.rs` | 15.2 KB | 411 |
| `javac-core\src\jvm\pool_constant.rs` | 10.8 KB | 352 |
| `javac-core\src\jvm\pool_reader.rs` | 27.3 KB | 664 |
| `javac-core\src\jvm\pool_writer.rs` | 28.7 KB | 781 |
| `javac-core\src\jvm\profile.rs` | 8.7 KB | 244 |
| `javac-core\src\jvm\stack_map.rs` | 13.1 KB | 376 |
| `javac-core\src\jvm\string_concat.rs` | 27.5 KB | 728 |
| `javac-core\src\jvm\target.rs` | 13.2 KB | 391 |
| `javac-core\src\jvm\uninitialized_type.rs` | 1.7 KB | 58 |
| `javac-core\src\launcher\mod.rs` | 97 B | 2 |
| `javac-core\src\lib.rs` | 469 B | 23 |
| `javac-core\src\main\mini_compiler.rs` | 38.4 KB | 993 |
| `javac-core\src\main\mod.rs` | 92 B | 5 |
| `javac-core\src\main\option_helper.rs` | 3.9 KB | 115 |
| `javac-core\src\main\option.rs` | 54.1 KB | 1374 |
| `javac-core\src\model\mod.rs` | 30 B | 1 |
| `javac-core\src\parser\doc_comment_parser.rs` | 72.6 KB | 1962 |
| `javac-core\src\parser\java_string_methods.rs` | 7.1 KB | 208 |
| `javac-core\src\parser\java_tokenizer.rs` | 47.4 KB | 1226 |
| `javac-core\src\parser\javac_parser.rs` | 273.4 KB | 6610 |
| `javac-core\src\parser\javadoc_tokenizer.rs` | 14.2 KB | 409 |
| `javac-core\src\parser\lazy_doc_comment_table.rs` | 8.1 KB | 248 |
| `javac-core\src\parser\lexer.rs` | 1.0 KB | 37 |
| `javac-core\src\parser\mod.rs` | 2.1 KB | 49 |
| `javac-core\src\parser\oracle_tests.rs` | 20.7 KB | 611 |
| `javac-core\src\parser\parser_factory.rs` | 3.8 KB | 120 |
| `javac-core\src\parser\parser.rs` | 1.1 KB | 30 |
| `javac-core\src\parser\reference_parser.rs` | 14.9 KB | 432 |
| `javac-core\src\parser\scanner_factory.rs` | 4.4 KB | 107 |
| `javac-core\src\parser\scanner.rs` | 3.0 KB | 101 |
| `javac-core\src\parser\text_block_support.rs` | 2.9 KB | 76 |
| `javac-core\src\parser\tokens.rs` | 21.3 KB | 611 |
| `javac-core\src\parser\unicode_reader.rs` | 20.6 KB | 609 |
| `javac-core\src\platform\ct_sym_java_file_manager.rs` | 20.2 KB | 585 |
| `javac-core\src\platform\jdk_platform_provider.rs` | 33.6 KB | 765 |
| `javac-core\src\platform\mod.rs` | 910 B | 21 |
| `javac-core\src\platform\platform_description.rs` | 7.6 KB | 194 |
| `javac-core\src\platform\platform_provider.rs` | 6.1 KB | 153 |
| `javac-core\src\platform\platform_utils.rs` | 10.1 KB | 246 |
| `javac-core\src\processing\mod.rs` | 96 B | 2 |
| `javac-core\src\resources\compiler_ja.rs` | 107.3 KB | 923 |
| `javac-core\src\resources\compiler_properties.rs` | 441.0 KB | 8581 |
| `javac-core\src\resources\compiler_zh_CN.rs` | 84.7 KB | 923 |
| `javac-core\src\resources\diagnostic_contract.rs` | 8.5 KB | 264 |
| `javac-core\src\resources\javac_ja.rs` | 16.4 KB | 153 |
| `javac-core\src\resources\javac_properties.rs` | 13.1 KB | 155 |
| `javac-core\src\resources\javac_zh_CN.rs` | 12.3 KB | 153 |
| `javac-core\src\resources\launcher_ja.rs` | 1.8 KB | 25 |
| `javac-core\src\resources\launcher_properties.rs` | 1.5 KB | 25 |
| `javac-core\src\resources\launcher_zh_CN.rs` | 1.5 KB | 25 |
| `javac-core\src\resources\mod.rs` | 5.1 KB | 143 |
| `javac-core\src\tree\dc_tree.rs` | 23.0 KB | 717 |
| `javac-core\src\tree\doc_comment_table.rs` | 6.5 KB | 200 |
| `javac-core\src\tree\doc_pretty.rs` | 33.2 KB | 1002 |
| `javac-core\src\tree\doc_tree_maker.rs` | 44.2 KB | 1190 |
| `javac-core\src\tree\end_pos_table.rs` | 3.5 KB | 106 |
| `javac-core\src\tree\jc_tree.rs` | 61.4 KB | 1829 |
| `javac-core\src\tree\mod.rs` | 281 B | 14 |
| `javac-core\src\tree\pretty.rs` | 89.2 KB | 2599 |
| `javac-core\src\tree\tree_copier.rs` | 74.0 KB | 2101 |
| `javac-core\src\tree\tree_info.rs` | 75.4 KB | 2222 |
| `javac-core\src\tree\tree_maker.rs` | 114.3 KB | 3274 |
| `javac-core\src\tree\tree_scanner.rs` | 71.0 KB | 1912 |
| `javac-core\src\tree\tree_translator.rs` | 45.9 KB | 1351 |
| `javac-core\src\util\abort.rs` | 1.2 KB | 48 |
| `javac-core\src\util\abstract_diagnostic_formatter.rs` | 24.0 KB | 584 |
| `javac-core\src\util\abstract_log.rs` | 21.0 KB | 593 |
| `javac-core\src\util\arena.rs` | 4.3 KB | 137 |
| `javac-core\src\util\array_utils.rs` | 3.4 KB | 97 |
| `javac-core\src\util\assert.rs` | 3.9 KB | 159 |
| `javac-core\src\util\basic_diagnostic_formatter.rs` | 33.3 KB | 900 |
| `javac-core\src\util\bits.rs` | 20.4 KB | 664 |
| `javac-core\src\util\byte_buffer.rs` | 13.6 KB | 388 |
| `javac-core\src\util\client_code_exception.rs` | 997 B | 31 |
| `javac-core\src\util\constants.rs` | 12.1 KB | 361 |
| `javac-core\src\util\context.rs` | 14.7 KB | 398 |
| `javac-core\src\util\convert.rs` | 14.7 KB | 451 |
| `javac-core\src\util\diagnostic_source.rs` | 16.8 KB | 458 |
| `javac-core\src\util\fatal_error.rs` | 1.6 KB | 47 |
| `javac-core\src\util\forwarding_diagnostic_formatter.rs` | 9.4 KB | 261 |
| `javac-core\src\util\graph_utils.rs` | 17.4 KB | 538 |
| `javac-core\src\util\int_hash_table.rs` | 10.3 KB | 300 |
| `javac-core\src\util\iterators.rs` | 8.3 KB | 239 |
| `javac-core\src\util\java_string.rs` | 329 B | 7 |
| `javac-core\src\util\javac_messages.rs` | 22.6 KB | 592 |
| `javac-core\src\util\jc_diagnostic.rs` | 54.4 KB | 1543 |
| `javac-core\src\util\layout_characters.rs` | 1.4 KB | 52 |
| `javac-core\src\util\list_buffer.rs` | 15.4 KB | 533 |
| `javac-core\src\util\list.rs` | 28.7 KB | 926 |
| `javac-core\src\util\log.rs` | 57.3 KB | 1504 |
| `javac-core\src\util\mandatory_warning_handler.rs` | 23.4 KB | 639 |
| `javac-core\src\util\message_format.rs` | 8.5 KB | 212 |
| `javac-core\src\util\mod.rs` | 927 B | 42 |
| `javac-core\src\util\module_helper.rs` | 2.3 KB | 60 |
| `javac-core\src\util\name.rs` | 12.0 KB | 357 |
| `javac-core\src\util\names.rs` | 13.9 KB | 447 |
| `javac-core\src\util\options.rs` | 15.3 KB | 393 |
| `javac-core\src\util\pair.rs` | 2.5 KB | 88 |
| `javac-core\src\util\position.rs` | 16.4 KB | 454 |
| `javac-core\src\util\propagated_exception.rs` | 1.2 KB | 35 |
| `javac-core\src\util\raw_diagnostic_formatter.rs` | 11.6 KB | 339 |
| `javac-core\src\util\shared_name_table.rs` | 5.2 KB | 136 |
| `javac-core\src\util\string_utils.rs` | 4.5 KB | 135 |
| `javac-core\src\util\unshared_name_table.rs` | 3.4 KB | 89 |
| `javac-core\src\util\warner.rs` | 4.3 KB | 126 |
| `javac-core\tests\data\attr-oracle.txt` | 301 B | 8 |
| `javac-core\tests\data\deferred-attr-oracle.txt` | 192 B | 5 |
| `javac-core\tests\data\enter-oracle.txt` | 1.4 KB | 21 |
| `javac-core\tests\data\fixtures\AnnotatedFixture.class` | 508 B | - |
| `javac-core\tests\data\fixtures\Circle.class` | 267 B | - |
| `javac-core\tests\data\fixtures\Fixtures.class` | 660 B | - |
| `javac-core\tests\data\fixtures\fixtures\p\Service.class` | 144 B | - |
| `javac-core\tests\data\fixtures\fixtures\p\ServiceImpl.class` | 314 B | - |
| `javac-core\tests\data\fixtures\Fixtures$1.class` | 469 B | - |
| `javac-core\tests\data\fixtures\Fixtures$1Local.class` | 427 B | - |
| `javac-core\tests\data\fixtures\Fixtures$Member.class` | 377 B | - |
| `javac-core\tests\data\fixtures\Fixtures$Nested.class` | 286 B | - |
| `javac-core\tests\data\fixtures\module-info.class` | 243 B | - |
| `javac-core\tests\data\fixtures\MyAnno.class` | 679 B | - |
| `javac-core\tests\data\fixtures\MyTypeAnno.class` | 381 B | - |
| `javac-core\tests\data\fixtures\RecordFixture.class` | 1.4 KB | - |
| `javac-core\tests\data\fixtures\SealedFixture.class` | 186 B | - |
| `javac-core\tests\data\fixtures\Square.class` | 278 B | - |
| `javac-core\tests\data\fixtures\TypeAnnotated.class` | 563 B | - |
| `javac-core\tests\data\infer-oracle.txt` | 394 B | 6 |
| `javac-core\tests\data\operators-oracle.txt` | 835 B | 35 |
| `javac-core\tests\data\resolve-b-oracle.txt` | 303 B | 11 |
| `javac-core\tests\data\resolve-error-oracle.txt` | 201 B | 8 |
| `javac-core\tests\data\resolve-oracle.txt` | 82 B | 4 |
| `javac-core\tests\data\types-oracle.txt` | 1.9 KB | 110 |
| `javac-core\tests\data\types-relations-oracle.txt` | 719 B | 47 |
| `javac-core\tests\e2e_compile.rs` | 9.4 KB | 292 |
| `javac-core\tests\module_uses_provides.rs` | 2.6 KB | 61 |
| `javac-core\tests\operators_oracle.rs` | 9.2 KB | 241 |
| `javac-core\tests\resolve_b_oracle.rs` | 2.8 KB | 65 |
| `javac-core\tests\tree_oracle.rs` | 7.8 KB | 215 |

## javac-ctsym — 2 个文件 / 63.0 KB / 1725 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-ctsym\Cargo.toml` | 160 B | 8 |
| `javac-ctsym\src\lib.rs` | 62.9 KB | 1717 |

## javac-floating — 10 个文件 / 361.3 KB / 10091 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-floating\Cargo.toml` | 171 B | 8 |
| `javac-floating\src\big_integer.rs` | 133.7 KB | 3809 |
| `javac-floating\src\digit_helper.rs` | 1.3 KB | 41 |
| `javac-floating\src\fdbig_integer.rs` | 43.3 KB | 1209 |
| `javac-floating\src\floating_decimal.rs` | 87.0 KB | 2316 |
| `javac-floating\src\lib.rs` | 2.3 KB | 57 |
| `javac-floating\src\mutable_big_integer.rs` | 78.5 KB | 2177 |
| `javac-floating\src\random.rs` | 5.2 KB | 159 |
| `javac-floating\tests\data\floating-oracle.txt` | 5.2 KB | 167 |
| `javac-floating\tests\floating_oracle.rs` | 4.6 KB | 148 |

## javac-fs — 5 个文件 / 50.4 KB / 1286 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-fs\Cargo.toml` | 119 B | 7 |
| `javac-fs\src\error.rs` | 2.1 KB | 58 |
| `javac-fs\src\files.rs` | 5.2 KB | 137 |
| `javac-fs\src\lib.rs` | 1.8 KB | 36 |
| `javac-fs\src\path.rs` | 41.2 KB | 1048 |

## javac-jimage — 6 个文件 / 72.4 KB / 1896 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-jimage\Cargo.toml` | 123 B | 7 |
| `javac-jimage\src\image_header.rs` | 4.6 KB | 143 |
| `javac-jimage\src\image_location.rs` | 8.4 KB | 273 |
| `javac-jimage\src\image_reader.rs` | 16.6 KB | 403 |
| `javac-jimage\src\image_strings.rs` | 5.6 KB | 166 |
| `javac-jimage\src\lib.rs` | 36.9 KB | 904 |

## javac-jmod — 2 个文件 / 19.3 KB / 520 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-jmod\Cargo.toml` | 159 B | 8 |
| `javac-jmod\src\lib.rs` | 19.1 KB | 512 |

## javac-source — 138 个文件 / 565.9 KB / 14208 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-source\Cargo.toml` | 351 B | 14 |
| `javac-source\src\doctree\attribute_tree.rs` | 2.2 KB | 69 |
| `javac-source\src\doctree\author_tree.rs` | 444 B | 19 |
| `javac-source\src\doctree\block_tag_tree.rs` | 395 B | 15 |
| `javac-source\src\doctree\comment_tree.rs` | 390 B | 18 |
| `javac-source\src\doctree\deprecated_tree.rs` | 503 B | 19 |
| `javac-source\src\doctree\doc_comment_tree.rs` | 2.9 KB | 75 |
| `javac-source\src\doctree\doc_root_tree.rs` | 283 B | 13 |
| `javac-source\src\doctree\doc_tree_visitor.rs` | 18.5 KB | 512 |
| `javac-source\src\doctree\doc_tree.rs` | 21.6 KB | 625 |
| `javac-source\src\doctree\doc_type_tree.rs` | 486 B | 20 |
| `javac-source\src\doctree\end_element_tree.rs` | 436 B | 19 |
| `javac-source\src\doctree\entity_tree.rs` | 488 B | 21 |
| `javac-source\src\doctree\erroneous_tree.rs` | 558 B | 18 |
| `javac-source\src\doctree\hidden_tree.rs` | 465 B | 19 |
| `javac-source\src\doctree\identifier_tree.rs` | 425 B | 19 |
| `javac-source\src\doctree\index_tree.rs` | 604 B | 23 |
| `javac-source\src\doctree\inherit_doc_tree.rs` | 295 B | 13 |
| `javac-source\src\doctree\inline_tag_tree.rs` | 398 B | 15 |
| `javac-source\src\doctree\link_tree.rs` | 691 B | 25 |
| `javac-source\src\doctree\literal_tree.rs` | 531 B | 20 |
| `javac-source\src\doctree\mod.rs` | 1.3 KB | 50 |
| `javac-source\src\doctree\param_tree.rs` | 842 B | 28 |
| `javac-source\src\doctree\provides_tree.rs` | 726 B | 24 |
| `javac-source\src\doctree\reference_tree.rs` | 560 B | 20 |
| `javac-source\src\doctree\return_tree.rs` | 1.0 KB | 34 |
| `javac-source\src\doctree\see_tree.rs` | 520 B | 21 |
| `javac-source\src\doctree\serial_data_tree.rs` | 493 B | 19 |
| `javac-source\src\doctree\serial_field_tree.rs` | 910 B | 29 |
| `javac-source\src\doctree\serial_tree.rs` | 549 B | 20 |
| `javac-source\src\doctree\since_tree.rs` | 484 B | 19 |
| `javac-source\src\doctree\start_element_tree.rs` | 835 B | 28 |
| `javac-source\src\doctree\summary_tree.rs` | 479 B | 19 |
| `javac-source\src\doctree\system_property_tree.rs` | 529 B | 19 |
| `javac-source\src\doctree\text_tree.rs` | 311 B | 14 |
| `javac-source\src\doctree\throws_tree.rs` | 846 B | 27 |
| `javac-source\src\doctree\unknown_block_tag_tree.rs` | 479 B | 19 |
| `javac-source\src\doctree\unknown_inline_tag_tree.rs` | 488 B | 19 |
| `javac-source\src\doctree\uses_tree.rs` | 707 B | 24 |
| `javac-source\src\doctree\value_tree.rs` | 473 B | 19 |
| `javac-source\src\doctree\version_tree.rs` | 448 B | 19 |
| `javac-source\src\lib.rs` | 1.7 KB | 35 |
| `javac-source\src\tree\annotated_type_tree.rs` | 876 B | 28 |
| `javac-source\src\tree\annotation_tree.rs` | 802 B | 29 |
| `javac-source\src\tree\array_access_tree.rs` | 722 B | 27 |
| `javac-source\src\tree\array_type_tree.rs` | 495 B | 23 |
| `javac-source\src\tree\assert_tree.rs` | 787 B | 30 |
| `javac-source\src\tree\assignment_tree.rs` | 738 B | 27 |
| `javac-source\src\tree\binary_tree.rs` | 1.1 KB | 35 |
| `javac-source\src\tree\binding_pattern_tree.rs` | 398 B | 15 |
| `javac-source\src\tree\block_tree.rs` | 760 B | 31 |
| `javac-source\src\tree\break_tree.rs` | 605 B | 26 |
| `javac-source\src\tree\case_label_tree.rs` | 240 B | 9 |
| `javac-source\src\tree\case_tree.rs` | 3.6 KB | 116 |
| `javac-source\src\tree\catch_tree.rs` | 874 B | 33 |
| `javac-source\src\tree\class_tree.rs` | 2.1 KB | 66 |
| `javac-source\src\tree\compilation_unit_tree.rs` | 3.2 KB | 78 |
| `javac-source\src\tree\compound_assignment_tree.rs` | 893 B | 28 |
| `javac-source\src\tree\conditional_expression_tree.rs` | 1.0 KB | 31 |
| `javac-source\src\tree\continue_tree.rs` | 622 B | 25 |
| `javac-source\src\tree\default_case_label_tree.rs` | 272 B | 9 |
| `javac-source\src\tree\directive_tree.rs` | 193 B | 7 |
| `javac-source\src\tree\do_while_loop_tree.rs` | 770 B | 30 |
| `javac-source\src\tree\empty_statement_tree.rs` | 395 B | 18 |
| `javac-source\src\tree\enhanced_for_loop_tree.rs` | 1.0 KB | 34 |
| `javac-source\src\tree\erroneous_tree.rs` | 483 B | 17 |
| `javac-source\src\tree\exports_tree.rs` | 909 B | 27 |
| `javac-source\src\tree\expression_statement_tree.rs` | 633 B | 24 |
| `javac-source\src\tree\expression_tree.rs` | 393 B | 15 |
| `javac-source\src\tree\for_loop_tree.rs` | 1.3 KB | 41 |
| `javac-source\src\tree\guarded_pattern_tree.rs` | 524 B | 19 |
| `javac-source\src\tree\identifier_tree.rs` | 579 B | 24 |
| `javac-source\src\tree\if_tree.rs` | 1.2 KB | 39 |
| `javac-source\src\tree\import_tree.rs` | 948 B | 33 |
| `javac-source\src\tree\instance_of_tree.rs` | 1.4 KB | 49 |
| `javac-source\src\tree\intersection_type_tree.rs` | 400 B | 16 |
| `javac-source\src\tree\labeled_statement_tree.rs` | 714 B | 28 |
| `javac-source\src\tree\lambda_expression_tree.rs` | 2.0 KB | 72 |
| `javac-source\src\tree\line_map.rs` | 1.6 KB | 43 |
| `javac-source\src\tree\literal_tree.rs` | 1.0 KB | 31 |
| `javac-source\src\tree\member_reference_tree.rs` | 2.1 KB | 72 |
| `javac-source\src\tree\member_select_tree.rs` | 880 B | 30 |
| `javac-source\src\tree\method_invocation_tree.rs` | 1.0 KB | 34 |
| `javac-source\src\tree\method_tree.rs` | 2.6 KB | 75 |
| `javac-source\src\tree\mod.rs` | 2.3 KB | 89 |
| `javac-source\src\tree\modifiers_tree.rs` | 1.0 KB | 37 |
| `javac-source\src\tree\module_tree.rs` | 2.1 KB | 76 |
| `javac-source\src\tree\new_array_tree.rs` | 1.7 KB | 52 |
| `javac-source\src\tree\new_class_tree.rs` | 1.6 KB | 49 |
| `javac-source\src\tree\opens_tree.rs` | 893 B | 27 |
| `javac-source\src\tree\package_tree.rs` | 698 B | 24 |
| `javac-source\src\tree\parameterized_type_tree.rs` | 700 B | 27 |
| `javac-source\src\tree\parenthesized_pattern_tree.rs` | 499 B | 21 |
| `javac-source\src\tree\parenthesized_tree.rs` | 641 B | 24 |
| `javac-source\src\tree\pattern_tree.rs` | 289 B | 11 |
| `javac-source\src\tree\primitive_type_tree.rs` | 609 B | 24 |
| `javac-source\src\tree\provides_tree.rs` | 814 B | 24 |
| `javac-source\src\tree\requires_tree.rs` | 983 B | 30 |
| `javac-source\src\tree\return_tree.rs` | 617 B | 25 |
| `javac-source\src\tree\scope.rs` | 2.1 KB | 47 |
| `javac-source\src\tree\statement_tree.rs` | 341 B | 14 |
| `javac-source\src\tree\switch_expression_tree.rs` | 739 B | 28 |
| `javac-source\src\tree\switch_tree.rs` | 826 B | 31 |
| `javac-source\src\tree\synchronized_tree.rs` | 842 B | 30 |
| `javac-source\src\tree\throw_tree.rs` | 596 B | 24 |
| `javac-source\src\tree\tree_visitor.rs` | 29.3 KB | 727 |
| `javac-source\src\tree\tree.rs` | 49.1 KB | 1334 |
| `javac-source\src\tree\try_tree.rs` | 1.4 KB | 47 |
| `javac-source\src\tree\type_cast_tree.rs` | 698 B | 28 |
| `javac-source\src\tree\type_parameter_tree.rs` | 1.2 KB | 44 |
| `javac-source\src\tree\unary_tree.rs` | 746 B | 27 |
| `javac-source\src\tree\union_type_tree.rs` | 452 B | 17 |
| `javac-source\src\tree\uses_tree.rs` | 524 B | 20 |
| `javac-source\src\tree\variable_tree.rs` | 1.6 KB | 47 |
| `javac-source\src\tree\while_loop_tree.rs` | 768 B | 30 |
| `javac-source\src\tree\wildcard_tree.rs` | 633 B | 28 |
| `javac-source\src\tree\yield_tree.rs` | 553 B | 23 |
| `javac-source\src\util\dispatch.rs` | 89.5 KB | 677 |
| `javac-source\src\util\doc_source_positions.rs` | 3.4 KB | 84 |
| `javac-source\src\util\doc_tree_factory.rs` | 33.1 KB | 798 |
| `javac-source\src\util\doc_tree_path_scanner.rs` | 13.3 KB | 356 |
| `javac-source\src\util\doc_tree_path.rs` | 10.1 KB | 240 |
| `javac-source\src\util\doc_tree_scanner.rs` | 13.8 KB | 375 |
| `javac-source\src\util\doc_trees.rs` | 7.1 KB | 163 |
| `javac-source\src\util\javac_task.rs` | 8.4 KB | 195 |
| `javac-source\src\util\mod.rs` | 1.1 KB | 36 |
| `javac-source\src\util\parameter_name_provider.rs` | 1.1 KB | 26 |
| `javac-source\src\util\plugin.rs` | 4.0 KB | 103 |
| `javac-source\src\util\simple_doc_tree_visitor.rs` | 21.5 KB | 641 |
| `javac-source\src\util\simple_tree_visitor.rs` | 34.9 KB | 1018 |
| `javac-source\src\util\source_positions.rs` | 3.0 KB | 75 |
| `javac-source\src\util\task_event.rs` | 7.5 KB | 217 |
| `javac-source\src\util\task_listener.rs` | 2.0 KB | 70 |
| `javac-source\src\util\test_support.rs` | 10.8 KB | 361 |
| `javac-source\src\util\tree_path_scanner.rs` | 29.9 KB | 777 |
| `javac-source\src\util\tree_path.rs` | 10.6 KB | 250 |
| `javac-source\src\util\tree_scanner.rs` | 30.6 KB | 782 |
| `javac-source\src\util\trees.rs` | 12.6 KB | 337 |

## javac-strings — 2 个文件 / 9.9 KB / 304 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-strings\Cargo.toml` | 124 B | 7 |
| `javac-strings\src\lib.rs` | 9.8 KB | 297 |

## javac-tests — 17 个文件 / 423.4 KB / 13124 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-tests\Cargo.toml` | 107 B | 6 |
| `javac-tests\data\diagnostics-builtin-cases.txt` | 6.9 KB | 328 |
| `javac-tests\data\diagnostics-oracle.txt` | 50.0 KB | 1675 |
| `javac-tests\data\token-oracle.txt` | 103.8 KB | 3679 |
| `javac-tests\data\tree-oracle.txt` | 41.2 KB | 1194 |
| `javac-tests\README.md` | 9.2 KB | 174 |
| `javac-tests\src\bin\diff_runner.rs` | 12.0 KB | 314 |
| `javac-tests\src\bin\gen_corpus_baselines.rs` | 5.2 KB | 135 |
| `javac-tests\src\bin\gen_diagnostics_oracle.rs` | 15.6 KB | 431 |
| `javac-tests\src\corpus.rs` | 43.9 KB | 1247 |
| `javac-tests\src\diagnostics_data.rs` | 17.7 KB | 565 |
| `javac-tests\src\diagnostics_oracle.rs` | 20.2 KB | 545 |
| `javac-tests\src\diff.rs` | 42.1 KB | 1200 |
| `javac-tests\src\javac.rs` | 16.2 KB | 467 |
| `javac-tests\src\lib.rs` | 7.6 KB | 225 |
| `javac-tests\src\normalize.rs` | 15.2 KB | 438 |
| `javac-tests\src\oracle_data.rs` | 16.5 KB | 501 |

## javac-tools — 120 个文件 / 600.3 KB / 15856 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-tools\Cargo.toml` | 395 B | 15 |
| `javac-tools\src\error.rs` | 3.1 KB | 83 |
| `javac-tools\src\java_io.rs` | 11.5 KB | 348 |
| `javac-tools\src\jdk.rs` | 4.4 KB | 124 |
| `javac-tools\src\lang\mod.rs` | 401 B | 10 |
| `javac-tools\src\lang\model\annotated_construct.rs` | 4.2 KB | 97 |
| `javac-tools\src\lang\model\element\annotation_mirror.rs` | 2.2 KB | 53 |
| `javac-tools\src\lang\model\element\annotation_value_visitor.rs` | 10.8 KB | 213 |
| `javac-tools\src\lang\model\element\annotation_value.rs` | 4.5 KB | 124 |
| `javac-tools\src\lang\model\element\element_kind.rs` | 6.4 KB | 213 |
| `javac-tools\src\lang\model\element\element_visitor.rs` | 7.5 KB | 162 |
| `javac-tools\src\lang\model\element\element.rs` | 9.2 KB | 238 |
| `javac-tools\src\lang\model\element\executable_element.rs` | 3.4 KB | 87 |
| `javac-tools\src\lang\model\element\mod.rs` | 7.1 KB | 158 |
| `javac-tools\src\lang\model\element\modifier.rs` | 5.2 KB | 164 |
| `javac-tools\src\lang\model\element\module_element.rs` | 13.7 KB | 368 |
| `javac-tools\src\lang\model\element\name.rs` | 2.9 KB | 69 |
| `javac-tools\src\lang\model\element\nesting_kind.rs` | 3.2 KB | 103 |
| `javac-tools\src\lang\model\element\package_element.rs` | 1.0 KB | 28 |
| `javac-tools\src\lang\model\element\parameterizable.rs` | 656 B | 20 |
| `javac-tools\src\lang\model\element\qualified_nameable.rs` | 481 B | 16 |
| `javac-tools\src\lang\model\element\record_component_element.rs` | 831 B | 23 |
| `javac-tools\src\lang\model\element\type_element.rs` | 4.1 KB | 99 |
| `javac-tools\src\lang\model\element\type_parameter_element.rs` | 1.6 KB | 41 |
| `javac-tools\src\lang\model\element\unknown_annotation_value_exception.rs` | 3.2 KB | 88 |
| `javac-tools\src\lang\model\element\unknown_directive_exception.rs` | 3.0 KB | 87 |
| `javac-tools\src\lang\model\element\unknown_element_exception.rs` | 3.3 KB | 94 |
| `javac-tools\src\lang\model\element\variable_element.rs` | 2.9 KB | 75 |
| `javac-tools\src\lang\model\mod.rs` | 2.3 KB | 53 |
| `javac-tools\src\lang\model\source_version.rs` | 24.2 KB | 658 |
| `javac-tools\src\lang\model\type_\array_type.rs` | 609 B | 20 |
| `javac-tools\src\lang\model\type_\declared_type.rs` | 2.2 KB | 55 |
| `javac-tools\src\lang\model\type_\error_type.rs` | 677 B | 18 |
| `javac-tools\src\lang\model\type_\executable_type.rs` | 2.8 KB | 73 |
| `javac-tools\src\lang\model\type_\intersection_type.rs` | 847 B | 24 |
| `javac-tools\src\lang\model\type_\mirrored_type_exception.rs` | 2.7 KB | 76 |
| `javac-tools\src\lang\model\type_\mirrored_types_exception.rs` | 3.5 KB | 93 |
| `javac-tools\src\lang\model\type_\mod.rs` | 2.3 KB | 70 |
| `javac-tools\src\lang\model\type_\no_type.rs` | 849 B | 23 |
| `javac-tools\src\lang\model\type_\null_type.rs` | 448 B | 16 |
| `javac-tools\src\lang\model\type_\primitive_type.rs` | 455 B | 16 |
| `javac-tools\src\lang\model\type_\reference_type.rs` | 471 B | 16 |
| `javac-tools\src\lang\model\type_\type_kind.rs` | 5.3 KB | 213 |
| `javac-tools\src\lang\model\type_\type_mirror.rs` | 5.5 KB | 161 |
| `javac-tools\src\lang\model\type_\type_variable.rs` | 1.8 KB | 49 |
| `javac-tools\src\lang\model\type_\type_visitor.rs` | 9.2 KB | 194 |
| `javac-tools\src\lang\model\type_\union_type.rs` | 483 B | 18 |
| `javac-tools\src\lang\model\type_\unknown_type_exception.rs` | 2.9 KB | 87 |
| `javac-tools\src\lang\model\type_\wildcard_type.rs` | 1.1 KB | 36 |
| `javac-tools\src\lang\model\unknown_entity_exception.rs` | 2.5 KB | 71 |
| `javac-tools\src\lang\model\util\abstract_annotation_value_visitor14.rs` | 1.2 KB | 24 |
| `javac-tools\src\lang\model\util\abstract_annotation_value_visitor6.rs` | 5.8 KB | 144 |
| `javac-tools\src\lang\model\util\abstract_annotation_value_visitor7.rs` | 1.2 KB | 24 |
| `javac-tools\src\lang\model\util\abstract_annotation_value_visitor8.rs` | 1.2 KB | 24 |
| `javac-tools\src\lang\model\util\abstract_annotation_value_visitor9.rs` | 1.2 KB | 24 |
| `javac-tools\src\lang\model\util\abstract_element_visitor14.rs` | 1.4 KB | 27 |
| `javac-tools\src\lang\model\util\abstract_element_visitor6.rs` | 9.4 KB | 239 |
| `javac-tools\src\lang\model\util\abstract_element_visitor7.rs` | 1.3 KB | 27 |
| `javac-tools\src\lang\model\util\abstract_element_visitor8.rs` | 1.3 KB | 27 |
| `javac-tools\src\lang\model\util\abstract_element_visitor9.rs` | 1.4 KB | 27 |
| `javac-tools\src\lang\model\util\abstract_type_visitor14.rs` | 1.1 KB | 24 |
| `javac-tools\src\lang\model\util\abstract_type_visitor6.rs` | 8.3 KB | 224 |
| `javac-tools\src\lang\model\util\abstract_type_visitor7.rs` | 1.2 KB | 24 |
| `javac-tools\src\lang\model\util\abstract_type_visitor8.rs` | 1.2 KB | 24 |
| `javac-tools\src\lang\model\util\abstract_type_visitor9.rs` | 1.1 KB | 24 |
| `javac-tools\src\lang\model\util\element_filter.rs` | 23.5 KB | 634 |
| `javac-tools\src\lang\model\util\element_kind_visitor14.rs` | 1.9 KB | 50 |
| `javac-tools\src\lang\model\util\element_kind_visitor6.rs` | 11.2 KB | 297 |
| `javac-tools\src\lang\model\util\element_kind_visitor7.rs` | 1.5 KB | 38 |
| `javac-tools\src\lang\model\util\element_kind_visitor8.rs` | 934 B | 23 |
| `javac-tools\src\lang\model\util\element_kind_visitor9.rs` | 1.3 KB | 33 |
| `javac-tools\src\lang\model\util\element_scanner14.rs` | 3.3 KB | 85 |
| `javac-tools\src\lang\model\util\element_scanner6.rs` | 8.2 KB | 201 |
| `javac-tools\src\lang\model\util\element_scanner7.rs` | 3.4 KB | 91 |
| `javac-tools\src\lang\model\util\element_scanner8.rs` | 993 B | 25 |
| `javac-tools\src\lang\model\util\element_scanner9.rs` | 1.6 KB | 36 |
| `javac-tools\src\lang\model\util\elements.rs` | 24.9 KB | 654 |
| `javac-tools\src\lang\model\util\mod.rs` | 2.5 KB | 73 |
| `javac-tools\src\lang\model\util\simple_annotation_value_visitor14.rs` | 1.1 KB | 24 |
| `javac-tools\src\lang\model\util\simple_annotation_value_visitor6.rs` | 7.4 KB | 209 |
| `javac-tools\src\lang\model\util\simple_annotation_value_visitor7.rs` | 1.1 KB | 24 |
| `javac-tools\src\lang\model\util\simple_annotation_value_visitor8.rs` | 1.1 KB | 24 |
| `javac-tools\src\lang\model\util\simple_annotation_value_visitor9.rs` | 1.1 KB | 24 |
| `javac-tools\src\lang\model\util\simple_element_visitor14.rs` | 1.4 KB | 32 |
| `javac-tools\src\lang\model\util\simple_element_visitor6.rs` | 6.1 KB | 159 |
| `javac-tools\src\lang\model\util\simple_element_visitor7.rs` | 2.8 KB | 74 |
| `javac-tools\src\lang\model\util\simple_element_visitor8.rs` | 1.0 KB | 25 |
| `javac-tools\src\lang\model\util\simple_element_visitor9.rs` | 1.0 KB | 24 |
| `javac-tools\src\lang\model\util\simple_type_visitor14.rs` | 917 B | 22 |
| `javac-tools\src\lang\model\util\simple_type_visitor6.rs` | 5.9 KB | 171 |
| `javac-tools\src\lang\model\util\simple_type_visitor7.rs` | 2.3 KB | 68 |
| `javac-tools\src\lang\model\util\simple_type_visitor8.rs` | 1.4 KB | 37 |
| `javac-tools\src\lang\model\util\simple_type_visitor9.rs` | 999 B | 24 |
| `javac-tools\src\lang\model\util\test_support.rs` | 13.4 KB | 416 |
| `javac-tools\src\lang\model\util\type_kind_visitor14.rs` | 830 B | 21 |
| `javac-tools\src\lang\model\util\type_kind_visitor6.rs` | 7.5 KB | 205 |
| `javac-tools\src\lang\model\util\type_kind_visitor7.rs` | 1.2 KB | 34 |
| `javac-tools\src\lang\model\util\type_kind_visitor8.rs` | 1.3 KB | 35 |
| `javac-tools\src\lang\model\util\type_kind_visitor9.rs` | 2.7 KB | 71 |
| `javac-tools\src\lang\model\util\types.rs` | 13.2 KB | 312 |
| `javac-tools\src\lib.rs` | 4.0 KB | 72 |
| `javac-tools\src\tools\diagnostic_collector.rs` | 3.9 KB | 122 |
| `javac-tools\src\tools\diagnostic_listener.rs` | 2.7 KB | 93 |
| `javac-tools\src\tools\diagnostic.rs` | 8.2 KB | 250 |
| `javac-tools\src\tools\documentation_tool.rs` | 9.3 KB | 242 |
| `javac-tools\src\tools\file_object.rs` | 8.0 KB | 205 |
| `javac-tools\src\tools\forwarding_file_object.rs` | 5.6 KB | 161 |
| `javac-tools\src\tools\forwarding_java_file_manager.rs` | 13.9 KB | 414 |
| `javac-tools\src\tools\forwarding_java_file_object.rs` | 6.4 KB | 181 |
| `javac-tools\src\tools\java_compiler.rs` | 18.4 KB | 432 |
| `javac-tools\src\tools\java_file_manager.rs` | 36.5 KB | 845 |
| `javac-tools\src\tools\java_file_object.rs` | 5.3 KB | 154 |
| `javac-tools\src\tools\mod.rs` | 2.9 KB | 74 |
| `javac-tools\src\tools\option_checker.rs` | 1.4 KB | 54 |
| `javac-tools\src\tools\simple_java_file_object.rs` | 15.6 KB | 391 |
| `javac-tools\src\tools\standard_java_file_manager.rs` | 30.1 KB | 784 |
| `javac-tools\src\tools\standard_location.rs` | 10.5 KB | 300 |
| `javac-tools\src\tools\tool_provider.rs` | 7.9 KB | 179 |
| `javac-tools\src\tools\tool.rs` | 3.8 KB | 112 |
| `javac-tools\src\uri.rs` | 12.5 KB | 369 |

## javac-unicode — 5 个文件 / 180.0 KB / 8217 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-unicode\Cargo.toml` | 124 B | 7 |
| `javac-unicode\src\generated.rs` | 149.6 KB | 7152 |
| `javac-unicode\src\lib.rs` | 27.6 KB | 984 |
| `javac-unicode\tests\char_oracle.rs` | 2.4 KB | 65 |
| `javac-unicode\tests\data\char-oracle.txt` | 364 B | 9 |

## javac-zip — 3 个文件 / 36.7 KB / 988 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `javac-zip\Cargo.toml` | 415 B | 12 |
| `javac-zip\src\cp437.rs` | 2.5 KB | 61 |
| `javac-zip\src\lib.rs` | 33.8 KB | 915 |

## rust-javac — 2 个文件 / 541 B / 22 行

| 文件 | 大小 | 行数 |
| --- | ---: | ---: |
| `rust-javac\Cargo.toml` | 199 B | 12 |
| `rust-javac\src\main.rs` | 342 B | 10 |
