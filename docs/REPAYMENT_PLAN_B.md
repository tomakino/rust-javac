# B-class repayment plan (javac-core first) — pending approval per file

Total unimplemented: 106.

## crates/javac-core/src/comp/resolve.rs (15)
- :196 fn=instance — unimplemented!("TODO(Context registration batch): Resolve.instance")
- :583 fn=check_accessible_type — unimplemented!(
- :1160 fn=? — unimplemented!(
- :2384 fn=load_class — unimplemented!(
- :2405 fn=load_class — unimplemented!(
- :2493 fn=lookup_package — unimplemented!("TODO(Modules batch): Resolve.lookupInvisibleSymbol")
- :2573 fn=check_restricted_type — unimplemented!("TODO(Log batch): Resolve.checkRestrictedType warning")
- :2843 fn=access_internal — unimplemented!(
- :3769 fn=report — unimplemented!(
- :4426 fn=? — unimplemented!(
- :4579 fn=most_specific — unimplemented!(
- :4626 fn=? — unimplemented!(
- :4975 fn=varargs_accessible — unimplemented!(
- :5541 fn=? — unimplemented!(
- :5604 fn=access — unimplemented!(

## crates/javac-core/src/jvm/class_reader.rs (9)
- :1123 fn=read_class_file — unimplemented!("TODO(P2/S2): readClassFile missingTypeVariables substitution (Types.subst/substBounds)");
- :1250 fn=read_class_buffer — unimplemented!("TODO(P2/Log): log.warning(BigMajorVersion) needs a DiagnosticPosition-carrying Log adapter");
- :1263 fn=read_class_buffer — unimplemented!("TODO(P2/Log): preview.disabledError(JavaFileObject,int) wiring is unimplemented (Log batch)");
- :1265 fn=read_class_buffer — unimplemented!("TODO(P2/Log): preview.warnPreview(JavaFileObject,int) wiring is unimplemented (Log batch)");
- :1534 fn=read_method — unimplemented!("TODO(P2/S2): readMethod this$n stripping (requires owner.members()/name checks and Code batch)");
- :1543 fn=read_method — unimplemented!("TODO(P2/Code batch): initParameterNames (Code.width/LocalVariableTable)");
- :2005 fn=read_type_to_proxy — unimplemented!("TODO(S2-B/ProxyType): ClassReader.ProxyType for module-info annotations");
- :2597 fn=accepts — unimplemented!("TODO(P2/Log): FutureAttr classfile lint warning (Log/Lint batch)");
- :3290 fn=bad_enclosing_method — unimplemented!("TODO(P2/S3): Fragments.BadEnclosingMethod(sym) accessor needs the Symbol argument (P4 contract)")

## crates/javac-core/src/comp/attr.rs (7)
- :1031 fn=visit_type_parameter — unimplemented!("TODO(Annotate/Types batch): Attr.visitTypeParameter")
- :1896 fn=visit_return — unimplemented!(
- :2632 fn=type_display — unimplemented!("TODO(Types/Symbol printing batch): Type.toString")
- :2651 fn=symbol_is_member_of — unimplemented!("TODO(Symbol/Types batch): Symbol.isMemberOf")
- :11387 fn=attrib — unimplemented!("TODO(Modules batch): Attr.attribModule (Attr.java:5144-5146)")
- :11390 fn=attrib — unimplemented!("TODO(Modules batch): Attr.attribPackage (Attr.java:5147-5149)")
- :14339 fn=report — unimplemented!("test check context report")

## crates/javac-core/src/code/preview.rs (6)
- :571 fn=to_uri — unimplemented!("test fixture")
- :577 fn=open_input_stream — unimplemented!("test fixture")
- :580 fn=open_output_stream — unimplemented!("test fixture")
- :586 fn=open_reader — unimplemented!("test fixture")
- :592 fn=get_char_content — unimplemented!("test fixture")
- :597 fn=open_writer — unimplemented!("test fixture")

## crates/javac-core/src/file/path_file_object.rs (6)
- :203 fn=infer_binary_name — unimplemented!(
- :444 fn=to_uri — unimplemented!(
- :449 fn=to_uri — unimplemented!("TODO(P2): JRTFileObject.toUri needs the jrt FileSystem")
- :484 fn=open_reader — unimplemented!(
- :491 fn=get_char_content — unimplemented!(
- :498 fn=open_writer — unimplemented!(

## crates/javac-core/src/code/types.rs (5)
- :496 fn=captured_name — unimplemented!(
- :1563 fn=? — unimplemented!(
- :5183 fn=glb_flattened — unimplemented!(
- :5519 fn=array_super_type — unimplemented!(
- :6546 fn=var_instantiated — unimplemented!("TODO(Infer batch): BoundsChangeRecorder.varInstantiated");

## crates/javac-core/src/comp/flow.rs (5)
- :1562 fn=transitive_covers — unimplemented!(
- :1575 fn=is_exhaustive — unimplemented!(
- :2936 fn=visit_new_class — unimplemented!(
- :2971 fn=visit_lambda — unimplemented!(
- :3065 fn=visit_try — unimplemented!(

## crates/javac-core/src/file/base_file_manager.rs (5)
- :250 fn=handle_option — unimplemented!(
- :295 fn=handle_options — unimplemented!(
- :307 fn=get_default_encoding_name — unimplemented!("TODO(charset batch): Charset.defaultCharset() is not ported")
- :331 fn=get_charset — Err(_) => unimplemented!(
- :354 fn=decode — Err(_) => unimplemented!(

## crates/javac-core/src/file/javac_file_manager.rs (4)
- :595 fn=get_container — unimplemented!(
- :886 fn=get_class_loader — unimplemented!(
- :941 fn=handle_option — unimplemented!(
- :1034 fn=close — unimplemented!("TODO(P6): BaseFileManager.deferredClose (daemon thread/wait semantics)");

## crates/javac-core/src/file/locations.rs (4)
- :868 fn=patch_modules_init — _ => unimplemented!(
- :904 fn=module_source_path_init — unimplemented!(
- :1299 fn=get_location_for_module — unimplemented!(
- :1383 fn=list_locations_for_modules — unimplemented!(

## crates/javac-core/src/parser/reference_parser.rs (4)
- :180 fn=parse_module — unimplemented!("TODO(parser S7): ReferenceParser::parseModule needs JavacParser")
- :186 fn=parse_type — unimplemented!("TODO(parser S7): ReferenceParser::parseType needs JavacParser")
- :192 fn=parse_member — unimplemented!("TODO(parser S7): ReferenceParser::parseMember needs JavacParser")
- :200 fn=parse_params — unimplemented!("TODO(parser S7): ReferenceParser::parseParams needs JavacParser")

## crates/javac-core/src/tree/tree_maker.rs (4)
- :2341 fn=is_unqualifiable — unimplemented!(
- :2454 fn=visit_constant_attribute — unimplemented!("TODO(literal value domain batch): Attribute.Constant Byte literal")
- :2457 fn=visit_constant_attribute — unimplemented!("TODO(literal value domain batch): Attribute.Constant Short literal")
- :2639 fn=literal_constant — ConstantValue::Byte(_) | ConstantValue::Short(_) => unimplemented!(

## crates/javac-core/src/comp/check.rs (3)
- :1217 fn=check_type — unimplemented!("TODO(Infer batch): Check.checkType free type listener");
- :1688 fn=check_redundant_cast — unimplemented!(
- :1839 fn=validate_annotation — unimplemented!(

## crates/javac-core/src/platform/platform_description.rs (3)
- :119 fn=get_file_manager — unimplemented!("test fixture")
- :151 fn=get_plugin — unimplemented!("test fixture")
- :165 fn=get_plugin — unimplemented!("test fixture")

## crates/javac-core/src/code/attribute.rs (2)
- :356 fn=type_display — unimplemented!("TODO(Types/Symbol printing batch): Type.toString")
- :360 fn=symbol_display — unimplemented!("TODO(Symbol printing batch): Symbol.toString")

## crates/javac-core/src/code/scope.rs (2)
- :1321 fn=? — unimplemented!(
- :1415 fn=? — unimplemented!(

## crates/javac-core/src/comp/enter.rs (2)
- :136 fn=get_class_env — unimplemented!("TODO(Lint batch): Enter.getClassEnv lintEnv.augment")
- :830 fn=unenter — unimplemented!("TODO(Enter batch): Enter.unenter (UnenterScanner)");

## crates/javac-core/src/file/relative_path.rs (2)
- :89 fn=resolve_against_fs — unimplemented!(
- :250 fn=get_zip_entry — unimplemented!("TODO(P2): RelativeFile.getZipEntry needs java.util.zip.ZipFile")

## crates/javac-core/src/tree/doc_tree_maker.rs (2)
- :275 fn=new_erroneous_tree — unimplemented!(
- :399 fn=new_reference_tree — unimplemented!("TODO(parser S8): newReferenceTree(String) needs ReferenceParser")

## crates/javac-floating/src/fdbig_integer.rs (2)
- :1090 fn=big5pow_rec — unimplemented!("TODO: java.math.BigInteger is not ported (FDBigInteger.toBigInteger)");
- :1097 fn=big5pow_rec — unimplemented!("TODO: java.math.BigInteger is not ported (FDBigInteger.toString)");

## crates/javac-core/src/code/class_finder.rs (1)
- :213 fn=get_supplementary_flags — unimplemented!("TODO(S3/JRTIndex): getSupplementaryFlags jrtIndex path")

## crates/javac-core/src/code/deferred_completion_failure_handler.rs (1)
- :319 fn=is_deferred_completer — unimplemented!("TODO(P3): isDeferredCompleter needs the completer trait downcast")

## crates/javac-core/src/code/symbol.rs (1)
- :448 fn=inited_metadata — unimplemented!("TODO(symbol arena batch): Symbol.initedMetadata (SymbolId of the symbol is required)")

## crates/javac-core/src/comp/annotate.rs (1)
- :453 fn=instance — unimplemented!(

## crates/javac-core/src/comp/argument_attr.rs (1)
- :1295 fn=? — unimplemented!(

## crates/javac-core/src/file/cache_fs_info.rs (1)
- :94 fn=get_jar_class_path — unimplemented!("TODO(P2): CacheFSInfo.getJarClassPath needs java.util.jar.JarFile/Manifest")

## crates/javac-core/src/file/fs_info.rs (1)
- :106 fn=get_jar_class_path — unimplemented!("TODO(P2): FSInfo.getJarClassPath needs java.util.jar.JarFile/Manifest")

## crates/javac-core/src/jvm/target.rs (1)
- :272 fn=instance — unimplemented!(

## crates/javac-core/src/platform/jdk_platform_provider.rs (1)
- :331 fn=get_file_manager — unimplemented!(

## crates/javac-core/src/platform/platform_provider.rs (1)
- :88 fn=get_file_manager — unimplemented!("test fixture")

## crates/javac-core/src/platform/platform_utils.rs (1)
- :112 fn=get_file_manager — unimplemented!("test fixture")

## crates/javac-floating/src/lib.rs (1)
- :18 fn=? — //!   corresponding methods are explicit unimplemented!(TODO).

## crates/javac-tools/src/tools/simple_java_file_object.rs (1)
- :141 fn=open_writer — unimplemented!(

## crates/javac-tools/src/tools/tool_provider.rs (1)
- :68 fn=get_system_java_compiler — unimplemented!(

