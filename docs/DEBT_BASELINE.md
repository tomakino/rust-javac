# debt baseline: unimplemented (bottom line)

Generated: 2026-10-26T20:57:14.822Z
Total unimplemented! lines: 104 (non-comment code sites: 95; comment/doc references: 9).
Bottom line: every non-comment site must be REALLY implemented (no panic substitution except section 31 interface-abstract rule), plus comment references cleaned. Target: 0.

## Non-comment code sites by file
- 14  crates/javac-core/src/comp/resolve.rs
- 7  crates/javac-core/src/comp/attr.rs
- 6  crates/javac-core/src/code/preview.rs
- 6  crates/javac-core/src/file/path_file_object.rs
- 5  crates/javac-core/src/code/types.rs
- 5  crates/javac-core/src/comp/flow.rs
- 5  crates/javac-core/src/file/base_file_manager.rs
- 4  crates/javac-core/src/file/javac_file_manager.rs
- 4  crates/javac-core/src/file/locations.rs
- 4  crates/javac-core/src/parser/reference_parser.rs
- 4  crates/javac-core/src/tree/tree_maker.rs
- 3  crates/javac-core/src/comp/check.rs
- 3  crates/javac-core/src/platform/platform_description.rs
- 2  crates/javac-core/src/code/scope.rs
- 2  crates/javac-core/src/comp/enter.rs
- 2  crates/javac-core/src/file/relative_path.rs
- 2  crates/javac-core/src/jvm/class_reader.rs
- 2  crates/javac-core/src/tree/doc_tree_maker.rs
- 2  crates/javac-floating/src/fdbig_integer.rs
- 1  crates/javac-core/src/code/class_finder.rs
- 1  crates/javac-core/src/code/deferred_completion_failure_handler.rs
- 1  crates/javac-core/src/code/symbol.rs
- 1  crates/javac-core/src/comp/annotate.rs
- 1  crates/javac-core/src/comp/argument_attr.rs
- 1  crates/javac-core/src/file/cache_fs_info.rs
- 1  crates/javac-core/src/file/fs_info.rs
- 1  crates/javac-core/src/jvm/target.rs
- 1  crates/javac-core/src/platform/jdk_platform_provider.rs
- 1  crates/javac-core/src/platform/platform_provider.rs
- 1  crates/javac-core/src/platform/platform_utils.rs
- 1  crates/javac-tools/src/tools/simple_java_file_object.rs
- 1  crates/javac-tools/src/tools/tool_provider.rs

## Non-comment code sites (full list)
1. crates/javac-core/src/code/class_finder.rs:216  unimplemented!("TODO(S3/JRTIndex): getSupplementaryFlags jrtIndex path")
2. crates/javac-core/src/code/deferred_completion_failure_handler.rs:319  unimplemented!("TODO(P3): isDeferredCompleter needs the completer trait downcast")
3. crates/javac-core/src/code/preview.rs:571  unimplemented!("test fixture")
4. crates/javac-core/src/code/preview.rs:577  unimplemented!("test fixture")
5. crates/javac-core/src/code/preview.rs:580  unimplemented!("test fixture")
6. crates/javac-core/src/code/preview.rs:586  unimplemented!("test fixture")
7. crates/javac-core/src/code/preview.rs:592  unimplemented!("test fixture")
8. crates/javac-core/src/code/preview.rs:597  unimplemented!("test fixture")
9. crates/javac-core/src/code/scope.rs:1321  unimplemented!(
10. crates/javac-core/src/code/scope.rs:1415  unimplemented!(
11. crates/javac-core/src/code/symbol.rs:448  unimplemented!("TODO(symbol arena batch): Symbol.initedMetadata (SymbolId of the symbol is required)")
12. crates/javac-core/src/code/types.rs:496  unimplemented!(
13. crates/javac-core/src/code/types.rs:1564  unimplemented!(
14. crates/javac-core/src/code/types.rs:5190  unimplemented!(
15. crates/javac-core/src/code/types.rs:5522  unimplemented!(
16. crates/javac-core/src/code/types.rs:6549  unimplemented!("TODO(Infer batch): BoundsChangeRecorder.varInstantiated");
17. crates/javac-core/src/comp/annotate.rs:453  unimplemented!(
18. crates/javac-core/src/comp/argument_attr.rs:1295  unimplemented!(
19. crates/javac-core/src/comp/attr.rs:1031  unimplemented!("TODO(Annotate/Types batch): Attr.visitTypeParameter")
20. crates/javac-core/src/comp/attr.rs:1896  unimplemented!(
21. crates/javac-core/src/comp/attr.rs:2632  unimplemented!("TODO(Types/Symbol printing batch): Type.toString")
22. crates/javac-core/src/comp/attr.rs:2651  unimplemented!("TODO(Symbol/Types batch): Symbol.isMemberOf")
23. crates/javac-core/src/comp/attr.rs:11387  unimplemented!("TODO(Modules batch): Attr.attribModule (Attr.java:5144-5146)")
24. crates/javac-core/src/comp/attr.rs:11390  unimplemented!("TODO(Modules batch): Attr.attribPackage (Attr.java:5147-5149)")
25. crates/javac-core/src/comp/attr.rs:14339  unimplemented!("test check context report")
26. crates/javac-core/src/comp/check.rs:1217  unimplemented!("TODO(Infer batch): Check.checkType free type listener");
27. crates/javac-core/src/comp/check.rs:1688  unimplemented!(
28. crates/javac-core/src/comp/check.rs:1839  unimplemented!(
29. crates/javac-core/src/comp/enter.rs:136  unimplemented!("TODO(Lint batch): Enter.getClassEnv lintEnv.augment")
30. crates/javac-core/src/comp/enter.rs:830  unimplemented!("TODO(Enter batch): Enter.unenter (UnenterScanner)");
31. crates/javac-core/src/comp/flow.rs:1562  unimplemented!(
32. crates/javac-core/src/comp/flow.rs:1575  unimplemented!(
33. crates/javac-core/src/comp/flow.rs:2936  unimplemented!(
34. crates/javac-core/src/comp/flow.rs:2971  unimplemented!(
35. crates/javac-core/src/comp/flow.rs:3065  unimplemented!(
36. crates/javac-core/src/comp/resolve.rs:196  unimplemented!("TODO(Context registration batch): Resolve.instance")
37. crates/javac-core/src/comp/resolve.rs:1179  unimplemented!(
38. crates/javac-core/src/comp/resolve.rs:2403  unimplemented!(
39. crates/javac-core/src/comp/resolve.rs:2424  unimplemented!(
40. crates/javac-core/src/comp/resolve.rs:2512  unimplemented!("TODO(Modules batch): Resolve.lookupInvisibleSymbol")
41. crates/javac-core/src/comp/resolve.rs:2592  unimplemented!("TODO(Log batch): Resolve.checkRestrictedType warning")
42. crates/javac-core/src/comp/resolve.rs:2862  unimplemented!(
43. crates/javac-core/src/comp/resolve.rs:3788  unimplemented!(
44. crates/javac-core/src/comp/resolve.rs:4445  unimplemented!(
45. crates/javac-core/src/comp/resolve.rs:4598  unimplemented!(
46. crates/javac-core/src/comp/resolve.rs:4645  unimplemented!(
47. crates/javac-core/src/comp/resolve.rs:4994  unimplemented!(
48. crates/javac-core/src/comp/resolve.rs:5560  unimplemented!(
49. crates/javac-core/src/comp/resolve.rs:5623  unimplemented!(
50. crates/javac-core/src/file/base_file_manager.rs:250  unimplemented!(
51. crates/javac-core/src/file/base_file_manager.rs:295  unimplemented!(
52. crates/javac-core/src/file/base_file_manager.rs:307  unimplemented!("TODO(charset batch): Charset.defaultCharset() is not ported")
53. crates/javac-core/src/file/base_file_manager.rs:331  Err(_) => unimplemented!(
54. crates/javac-core/src/file/base_file_manager.rs:354  Err(_) => unimplemented!(
55. crates/javac-core/src/file/cache_fs_info.rs:94  unimplemented!("TODO(P2): CacheFSInfo.getJarClassPath needs java.util.jar.JarFile/Manifest")
56. crates/javac-core/src/file/fs_info.rs:106  unimplemented!("TODO(P2): FSInfo.getJarClassPath needs java.util.jar.JarFile/Manifest")
57. crates/javac-core/src/file/javac_file_manager.rs:595  unimplemented!(
58. crates/javac-core/src/file/javac_file_manager.rs:886  unimplemented!(
59. crates/javac-core/src/file/javac_file_manager.rs:941  unimplemented!(
60. crates/javac-core/src/file/javac_file_manager.rs:1034  unimplemented!("TODO(P6): BaseFileManager.deferredClose (daemon thread/wait semantics)");
61. crates/javac-core/src/file/locations.rs:868  _ => unimplemented!(
62. crates/javac-core/src/file/locations.rs:904  unimplemented!(
63. crates/javac-core/src/file/locations.rs:1299  unimplemented!(
64. crates/javac-core/src/file/locations.rs:1383  unimplemented!(
65. crates/javac-core/src/file/path_file_object.rs:203  unimplemented!(
66. crates/javac-core/src/file/path_file_object.rs:444  unimplemented!(
67. crates/javac-core/src/file/path_file_object.rs:449  unimplemented!("TODO(P2): JRTFileObject.toUri needs the jrt FileSystem")
68. crates/javac-core/src/file/path_file_object.rs:484  unimplemented!(
69. crates/javac-core/src/file/path_file_object.rs:491  unimplemented!(
70. crates/javac-core/src/file/path_file_object.rs:498  unimplemented!(
71. crates/javac-core/src/file/relative_path.rs:89  unimplemented!(
72. crates/javac-core/src/file/relative_path.rs:250  unimplemented!("TODO(P2): RelativeFile.getZipEntry needs java.util.zip.ZipFile")
73. crates/javac-core/src/jvm/class_reader.rs:1630  unimplemented!("TODO(P2/S2): readMethod this$n stripping (requires owner.members()/name checks and Code batch)");
74. crates/javac-core/src/jvm/class_reader.rs:1639  unimplemented!("TODO(P2/Code batch): initParameterNames (Code.width/LocalVariableTable)");
75. crates/javac-core/src/jvm/target.rs:272  unimplemented!(
76. crates/javac-core/src/parser/reference_parser.rs:180  unimplemented!("TODO(parser S7): ReferenceParser::parseModule needs JavacParser")
77. crates/javac-core/src/parser/reference_parser.rs:186  unimplemented!("TODO(parser S7): ReferenceParser::parseType needs JavacParser")
78. crates/javac-core/src/parser/reference_parser.rs:192  unimplemented!("TODO(parser S7): ReferenceParser::parseMember needs JavacParser")
79. crates/javac-core/src/parser/reference_parser.rs:200  unimplemented!("TODO(parser S7): ReferenceParser::parseParams needs JavacParser")
80. crates/javac-core/src/platform/jdk_platform_provider.rs:331  unimplemented!(
81. crates/javac-core/src/platform/platform_description.rs:119  unimplemented!("test fixture")
82. crates/javac-core/src/platform/platform_description.rs:151  unimplemented!("test fixture")
83. crates/javac-core/src/platform/platform_description.rs:165  unimplemented!("test fixture")
84. crates/javac-core/src/platform/platform_provider.rs:88  unimplemented!("test fixture")
85. crates/javac-core/src/platform/platform_utils.rs:112  unimplemented!("test fixture")
86. crates/javac-core/src/tree/doc_tree_maker.rs:275  unimplemented!(
87. crates/javac-core/src/tree/doc_tree_maker.rs:399  unimplemented!("TODO(parser S8): newReferenceTree(String) needs ReferenceParser")
88. crates/javac-core/src/tree/tree_maker.rs:2341  unimplemented!(
89. crates/javac-core/src/tree/tree_maker.rs:2454  unimplemented!("TODO(literal value domain batch): Attribute.Constant Byte literal")
90. crates/javac-core/src/tree/tree_maker.rs:2457  unimplemented!("TODO(literal value domain batch): Attribute.Constant Short literal")
91. crates/javac-core/src/tree/tree_maker.rs:2639  ConstantValue::Byte(_) | ConstantValue::Short(_) => unimplemented!(
92. crates/javac-floating/src/fdbig_integer.rs:1090  unimplemented!("TODO: java.math.BigInteger is not ported (FDBigInteger.toBigInteger)");
93. crates/javac-floating/src/fdbig_integer.rs:1097  unimplemented!("TODO: java.math.BigInteger is not ported (FDBigInteger.toString)");
94. crates/javac-tools/src/tools/simple_java_file_object.rs:141  unimplemented!(
95. crates/javac-tools/src/tools/tool_provider.rs:68  unimplemented!(

## Comment/doc references to clean
- crates/javac-core/src/comp/attr.rs:1545  // registered with unimplemented! + TODO at the exact call site.
- crates/javac-core/src/comp/resolve.rs:40  //! NOT YET PORTED IN THIS FILE (registered; each with unimplemented! + TODO):
- crates/javac-core/src/jvm/class_reader.rs:75  //! * NOT ported yet (unimplemented! with TODO below): readClass and the
- crates/javac-core/src/parser/doc_comment_parser.rs:31  //!   diagSource, code), which is still unimplemented! in tree/doc_tree_maker.rs
- crates/javac-core/src/parser/doc_comment_parser.rs:36  //!   ReferenceParser unimplemented! marker until parser S7 lands.
- crates/javac-core/src/parser/doc_comment_parser.rs:929  // is still unimplemented! (the upstream diagnostic position is the node
- crates/javac-core/src/parser/reference_parser.rs:18  //! check are left unimplemented! with TODO(parser S7) markers. No replacement
- crates/javac-core/src/tree/doc_tree_maker.rs:25  //!   unimplemented! (registered, no replacement algorithm is written);
- crates/javac-floating/src/lib.rs:18  //!   corresponding methods are explicit `unimplemented!(TODO)`.
