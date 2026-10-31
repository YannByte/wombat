/** Source-only copy and key checks. Source content and open protocol values are not translations. */
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

const han = /[\u3400-\u9fff]/u;
const copyProperties = /^(label|title|description|placeholder|footer|emptyLabel|message|aria-label|aria-description|alt)$/;
// Names, units and command identifiers retain their spelling. This is an exact token list, not an English exemption.
const unchangedTokens = new Set(['Token', 'USD', 'KiB', 'B', 'ms', 'Codex', 'Wombat', 'Agent', 'MCP', 'Skill', 'Hooks', 'AGENTS.md', 'SKILL.md', 'English', 'YYYY-MM-DD']);
const unchanged = (value: string) => {
  if (unchangedTokens.has(value.trim())) return true;
  const token = value.replace(/\b\d+M\b/g, '').replace(/[^A-Za-z\u3400-\u9fff]/gu, '');
  return !token || unchangedTokens.has(token);
};
// Exact dictionary-owned technical labels; neither whole namespaces nor arbitrary Latin text are exempt.
const chineseTechnicalEntries: Readonly<Record<string, string>> = {
  'preview.skillRead': 'SKILL.md · {status}',
  'directories.cliHelp': 'wombat directories list [--json]\nwombat directories authorize --purpose source|project --path <directory> [--json]\nwombat directories revoke --grant <id> [--json]\n',
  'config.hook': 'Hooks', 'config.rule': 'AGENTS.md', 'config.skill': 'Skill', 'config.mcp': 'MCP',
  'webui.agent': 'Agent', 'execution.useKind.skill': 'Skill', 'execution.useKind.mcp': 'MCP',
};
const location = (source: ts.SourceFile, node: ts.Node) => `${source.fileName}:${source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1}`;

function copySink(node: ts.Node): boolean {
  let current = node;
  while (current.parent) {
    const parent = current.parent;
    if (ts.isPropertyAssignment(parent) && parent.initializer === current) {
      return copyProperties.test(ts.isIdentifier(parent.name) || ts.isStringLiteral(parent.name) ? parent.name.text : '');
    }
    if (ts.isParenthesizedExpression(parent) || ts.isAsExpression(parent) || ts.isSatisfiesExpression(parent)
      || ts.isTemplateSpan(parent) || ts.isTemplateExpression(parent)
      || ts.isConditionalExpression(parent) && parent.condition !== current
      || ts.isBinaryExpression(parent) && parent.operatorToken.kind === ts.SyntaxKind.PlusToken) current = parent;
    else return false;
  }
  return false;
}
export function findCopy(file: string, content: string, context?: { source: ts.SourceFile; checker: ts.TypeChecker }): string[] {
  const source = context?.source ?? ts.createSourceFile(file, content, ts.ScriptTarget.Latest, true);
  const host = ts.createCompilerHost({ noLib: true, noResolve: true });
  host.getSourceFile = name => name === file ? source : undefined;
  const checker = context?.checker ?? ts.createProgram([file], { noLib: true, noResolve: true }, host).getTypeChecker();
  const errors: string[] = [];
  // Follow only this file's scalar constants and no-argument helper returns. Data properties, parameters,
  // external calls and mutable variables retain their source/protocol meaning; this is not a data-flow analysis.
  function inspectExpression(root: ts.Expression): void {
    const seen = new Set<ts.Node>();
    let work = 0;
    let limited = false;
    const limit = () => { if (!limited) { limited = true; errors.push(`${location(source, root)}: static copy inspection limit reached; resolve product labels through locale keys`); } };
    function trace(node: ts.Node, depth: number): void {
      if (seen.has(node)) return;
      if (depth > 8 || work++ >= 512) { limit(); return; }
      seen.add(node);
      if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node) || ts.isTemplateExpression(node)) {
        const value = ts.isTemplateExpression(node) ? node.head.text + node.templateSpans.map(span => span.literal.text).join('') : node.text;
        if (/[A-Za-z\u3400-\u9fff]/u.test(value) && !unchanged(value)) errors.push(`${location(source, node)}: move product copy ${JSON.stringify(value.slice(0, 100))} into the locale dictionary`);
        if (ts.isTemplateExpression(node)) for (const span of node.templateSpans) trace(span.expression, depth + 1);
      } else if (ts.isIdentifier(node)) {
        for (const declaration of checker.getSymbolAtLocation(node)?.declarations ?? []) {
          if (declaration.getSourceFile() === source && ts.isVariableDeclaration(declaration) && declaration.initializer
            && ts.isVariableDeclarationList(declaration.parent) && declaration.parent.flags & ts.NodeFlags.Const) trace(declaration.initializer, depth + 1);
        }
      } else if (ts.isCallExpression(node) && node.arguments.length === 0) {
        const declaration = checker.getResolvedSignature(node)?.declaration;
        if (declaration?.getSourceFile() === source && (ts.isFunctionDeclaration(declaration) || ts.isArrowFunction(declaration) || ts.isFunctionExpression(declaration)) && declaration.body) {
          if (!ts.isBlock(declaration.body)) trace(declaration.body, depth + 1);
          else {
            const returns = (child: ts.Node): void => {
              if (work++ >= 512) { limit(); return; }
              if (ts.isReturnStatement(child) && child.expression) trace(child.expression, depth + 1);
              else if (!ts.isFunctionLike(child)) ts.forEachChild(child, returns);
            };
            ts.forEachChild(declaration.body, returns);
          }
        }
      } else if (ts.isConditionalExpression(node)) {
        trace(node.whenTrue, depth + 1); trace(node.whenFalse, depth + 1);
      } else if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.PlusToken) {
        trace(node.left, depth + 1); trace(node.right, depth + 1);
      } else if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node)) trace(node.expression, depth + 1);
    }
    trace(root, 0);
  }
  function visit(node: ts.Node): void {
    if (ts.isPropertyAssignment(node) && copySink(node.initializer)) inspectExpression(node.initializer);
    const literal = ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node);
    const value = literal ? node.text : ts.isTemplateExpression(node) ? node.head.text + node.templateSpans.map(span => span.literal.text).join('') : undefined;
    // Only presentation sinks are inspected. Raw source fixtures, codes, identifiers and comments can contain any language.
    if (value !== undefined && copySink(node) && /[A-Za-z\u3400-\u9fff]/u.test(value) && !unchanged(value) && value !== 'YYYY-MM-DD') {
      errors.push(`${location(source, node)}: move product copy ${JSON.stringify(value.slice(0, 100))} into the locale dictionary`);
    }
    if (ts.isCallExpression(node) && node.expression.getText(source) === 't') {
      let parent: ts.Node | undefined = node.parent;
      while (parent && !ts.isFunctionLike(parent) && !ts.isSourceFile(parent)) parent = parent.parent;
      if (parent && ts.isSourceFile(parent)) errors.push(`${location(source, node)}: translation captured during module initialization; resolve labels lazily`);
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
  return [...new Set(errors)];
}
function files(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap(entry => entry.isDirectory() ? files(path.join(dir, entry.name)) : /\.tsx?$/.test(entry.name) ? [path.join(dir, entry.name)] : []);
}
export function readDictionary(file: string, content: string): { entries: Record<string, string>; errors: string[] } {
  const source = ts.createSourceFile(file, content, ts.ScriptTarget.Latest, true);
  const entries: Record<string, string> = Object.create(null);
  const errors: string[] = [];
  const name = path.basename(file, '.ts');
  let found = false;
  for (const statement of source.statements) {
    if (!ts.isVariableStatement(statement)) continue;
    for (const declaration of statement.declarationList.declarations) {
      if (!ts.isIdentifier(declaration.name) || declaration.name.text !== name) continue;
      found = true;
      if (!statement.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.ExportKeyword)) errors.push(`${file}: ${name} dictionary must be exported`);
      let initializer = declaration.initializer;
      while (initializer && (ts.isAsExpression(initializer) || ts.isSatisfiesExpression(initializer) || ts.isParenthesizedExpression(initializer))) initializer = initializer.expression;
      if (!initializer || !ts.isObjectLiteralExpression(initializer)) { errors.push(`${file}: expected a literal ${name} dictionary`); continue; }
      for (const property of initializer.properties) {
        if (!ts.isPropertyAssignment(property) || !(ts.isIdentifier(property.name) || ts.isStringLiteral(property.name)) || !ts.isStringLiteral(property.initializer)) {
          errors.push(`${location(source, property)}: dictionary entries must have literal keys and string values`); continue;
        }
        const key = property.name.text;
        if (Object.hasOwn(entries, key)) errors.push(`${location(source, property)}: ${key}: duplicate locale entry`);
        entries[key] = property.initializer.text;
      }
    }
  }
  if (found && !Object.keys(entries).length) errors.push(`${file}: dictionary has no literal entries`);
  if (!found) errors.push(`${file}: missing ${name} dictionary`);
  return { entries, errors };
}
export function dictionaryErrors(zh: Record<string, string>, en: Record<string, string>): string[] {
  const errors: string[] = [];
  const slots = (value: string) => [...new Set(value.match(/\{\w+\}/g) ?? [])].sort().join();
  for (const key of new Set([...Object.keys(zh), ...Object.keys(en)])) {
    if (!Object.hasOwn(zh, key) || !Object.hasOwn(en, key)) errors.push(`${key}: missing locale entry`);
    else {
      if (!zh[key].trim() || !en[key].trim()) errors.push(`${key}: empty locale entry`);
      if (slots(zh[key]) !== slots(en[key])) errors.push(`${key}: interpolation parameters differ`);
      // The language selector deliberately shows the target language's own name.
      if (han.test(en[key]) && !(key === 'webui.languageChinese' && en[key] === '中文')) errors.push(`${key}: Chinese text in English locale entry`);
      const staticChinese = zh[key].replace(/\{\w+\}/g, '');
      if (!han.test(staticChinese) && /[A-Za-z]/.test(staticChinese) && chineseTechnicalEntries[key] !== zh[key]) {
        errors.push(`${key}: ${zh[key] === en[key] ? 'untranslated English text shared by both locales' : 'untranslated English text in Chinese locale entry'}`);
      }
    }
  }
  return errors;
}

/** Resolve finite translation key types against source DTOs; new closed status variants cannot fall through to a raw key. */
export function translationKeyErrors(program: ts.Program, dictionary: Record<string, string>, localeFile: string, sourceFiles: readonly string[]): string[] {
  const checker = program.getTypeChecker();
  const errors: string[] = [];
  const keys = (type: ts.Type): string[] | undefined => {
    if (type.isStringLiteral()) return [type.value];
    if (type.isUnion()) {
      const parts = type.types.map(keys);
      return parts.every(part => part !== undefined) ? parts.flat() : undefined;
    }
    const constraint = checker.getBaseConstraintOfType(type);
    return constraint && constraint !== type ? keys(constraint) : undefined;
  };
  for (const file of sourceFiles) {
    const loaded = program.getSourceFile(file);
    if (!loaded) { errors.push(`${file}: source missing from translation check`); continue; }
    const source = loaded;
    function importedFrom(symbol: ts.Symbol | undefined, module: string, exported?: string): boolean {
      return !!symbol?.declarations?.some(declaration => {
        if (!(ts.isImportSpecifier(declaration) || ts.isImportClause(declaration) || ts.isNamespaceImport(declaration))) return false;
        if (exported && (!ts.isImportSpecifier(declaration) || (declaration.propertyName ?? declaration.name).text !== exported)) return false;
        let parent: ts.Node | undefined = declaration;
        while (parent && !ts.isImportDeclaration(parent)) parent = parent.parent;
        return !!parent && ts.isImportDeclaration(parent) && ts.isStringLiteral(parent.moduleSpecifier) && parent.moduleSpecifier.text === module;
      });
    }
    const ownedCall = (call: ts.CallExpression) => {
      let root: ts.Expression = call.expression;
      while (ts.isPropertyAccessExpression(root)) root = root.expression;
      return ts.isIdentifier(root) && importedFrom(checker.getSymbolAtLocation(root), '@wombat/client/locale');
    };
    for (const statement of source.statements) {
      if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier) || statement.moduleSpecifier.text !== '@wombat/client/locale') continue;
      const module = checker.getSymbolAtLocation(statement.moduleSpecifier);
      if (!module) { errors.push(`${location(source, statement)}: locale module could not be resolved`); continue; }
      const exports = new Set(checker.getExportsOfModule(module).map(symbol => symbol.name));
      const named = statement.importClause?.namedBindings;
      if (named && ts.isNamedImports(named)) for (const imported of named.elements) {
        const name = (imported.propertyName ?? imported.name).text;
        if (!exports.has(name)) errors.push(`${location(source, imported)}: ${name}: locale export could not be resolved`);
      }
    }
    function visit(node: ts.Node): void {
      if (ts.isCallExpression(node)) {
        if (ownedCall(node) && !checker.getResolvedSignature(node)?.declaration) errors.push(`${location(source, node)}: locale call could not be resolved`);
      }
      if (ts.isCallExpression(node) && node.arguments[0]) {
        const declaration = checker.getResolvedSignature(node)?.declaration;
        if (declaration && path.resolve(declaration.getSourceFile().fileName) === path.resolve(localeFile)) {
          let ancestor: ts.Node | undefined = declaration;
          while (ancestor && !ts.isTypeAliasDeclaration(ancestor) && !ts.isFunctionDeclaration(ancestor)) ancestor = ancestor.parent;
          const translate = ancestor && ts.isTypeAliasDeclaration(ancestor) && ancestor.name.text === 'Translate';
          const labels = ancestor && ts.isFunctionDeclaration(ancestor) && ancestor.name?.text === 'labels';
          const types = translate ? [checker.getTypeAtLocation(node.arguments[0])] : labels
            ? checker.getPropertiesOfType(checker.getTypeAtLocation(node.arguments[0])).map(property => checker.getTypeOfSymbolAtLocation(property, node.arguments[0])) : [];
          for (const type of types) {
            const resolved = keys(type);
            if (!resolved) errors.push(`${location(source, node)}: translation key type is not finite; map known values and handle unknown source values separately`);
            else for (const key of new Set(resolved)) if (!Object.hasOwn(dictionary, key)) errors.push(`${location(source, node)}: ${key}: translation key has no locale entry`);
          }
          if (translate && node.expression.getText(source) !== 't') {
            let parent: ts.Node | undefined = node.parent;
            while (parent && !ts.isFunctionLike(parent) && !ts.isSourceFile(parent)) parent = parent.parent;
            if (parent && ts.isSourceFile(parent)) errors.push(`${location(source, node)}: translation captured during module initialization; resolve labels lazily`);
          }
        }
      }
      ts.forEachChild(node, visit);
    }
    visit(source);
  }
  return errors;
}
export function checkProject(root: string): string[] {
  const localeFile = path.join(root, 'client/src/locale/index.ts');
  const localeDir = path.dirname(localeFile);
  const zhFile = path.join(localeDir, 'zh.ts'), enFile = path.join(localeDir, 'en.ts');
  const zh = readDictionary(zhFile, readFileSync(zhFile, 'utf8')), en = readDictionary(enFile, readFileSync(enFile, 'utf8'));
  const sourceFiles = [...files(path.join(root, 'cli/src')), localeFile];
  const program = ts.createProgram([...sourceFiles, zhFile, enFile], {
    target: ts.ScriptTarget.ES2024, module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext,
    strict: true, skipLibCheck: true, noEmit: true,
    paths: {
      '@wombat/client': [path.join(root, 'client/src/index.ts')],
      '@wombat/client/locale': [localeFile],
      '@wombat/client/node': [path.join(root, 'client/src/node/index.ts')],
    },
  });
  const syntaxErrors = program.getSyntacticDiagnostics().map(diagnostic => {
    const line = diagnostic.file && diagnostic.start !== undefined ? `${diagnostic.file.fileName}:${diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start).line + 1}: ` : '';
    return line + ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n');
  });
  return [...zh.errors, ...en.errors, ...syntaxErrors, ...dictionaryErrors(zh.entries, en.entries),
    ...sourceFiles.filter(file => file !== localeFile).flatMap(file => findCopy(file, readFileSync(file, 'utf8'), { source: program.getSourceFile(file)!, checker: program.getTypeChecker() })),
    ...translationKeyErrors(program, zh.entries, localeFile, sourceFiles)];
}
if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  try {
    if (process.argv.length > 3) throw new Error('Usage: check-copy.ts [root]');
    const errors = checkProject(path.resolve(process.argv[2] ?? '.'));
    if (errors.length) { console.error(errors.join('\n')); process.exitCode = 1; }
    else console.log('Product locale dictionaries, finite translation keys, parameters and copy ownership passed.');
  } catch (error) { console.error(error instanceof Error ? error.message : String(error)); process.exitCode = 1; }
}
