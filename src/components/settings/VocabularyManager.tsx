import React, { useState, useEffect, useCallback } from "react";
import { toast } from "sonner";
import { commands } from "@/bindings";
import type { VocabTerm, CorrectionEntry } from "@/bindings";
import { Input } from "../ui/Input";
import { Button } from "../ui/Button";
import { Textarea } from "../ui/Textarea";
import { SettingsGroup } from "../ui/SettingsGroup";

type Tab = "terms" | "corrections";

export const VocabularyManager: React.FC = React.memo(() => {
  const [activeTab, setActiveTab] = useState<Tab>("terms");
  const [terms, setTerms] = useState<VocabTerm[]>([]);
  const [termsLoading, setTermsLoading] = useState(false);
  const [search, setSearch] = useState("");
  const [newTerm, setNewTerm] = useState("");
  const [addingTerm, setAddingTerm] = useState(false);
  const [showImport, setShowImport] = useState(false);
  const [importText, setImportText] = useState("");
  const [importing, setImporting] = useState(false);
  const [corrections, setCorrections] = useState<CorrectionEntry[]>([]);
  const [correctionsLoaded, setCorrectionsLoaded] = useState(false);
  const [correctionsLoading, setCorrectionsLoading] = useState(false);

  const loadTerms = useCallback(async () => {
    setTermsLoading(true);
    try {
      setTerms(await commands.vocabListTerms());
    } catch (e) {
      toast.error(`Failed to load terms: ${e}`);
    } finally {
      setTermsLoading(false);
    }
  }, []);

  const loadCorrections = useCallback(async () => {
    if (correctionsLoaded) return;
    setCorrectionsLoading(true);
    try {
      setCorrections(await commands.vocabListCorrections(null));
      setCorrectionsLoaded(true);
    } catch (e) {
      toast.error(`Failed to load corrections: ${e}`);
    } finally {
      setCorrectionsLoading(false);
    }
  }, [correctionsLoaded]);

  useEffect(() => { loadTerms(); }, [loadTerms]);
  useEffect(() => { if (activeTab === "corrections") loadCorrections(); }, [activeTab, loadCorrections]);

  const handleAddTerm = async () => {
    const trimmed = newTerm.trim();
    if (!trimmed) return;
    setAddingTerm(true);
    try {
      const added = await commands.vocabAddTerm(trimmed);
      setTerms((prev) => [added, ...prev]);
      setNewTerm("");
    } catch (e) {
      toast.error(`Failed to add term: ${e}`);
    } finally {
      setAddingTerm(false);
    }
  };

  const handleSuppress = async (id: number) => {
    try {
      await commands.vocabSuppressTerm(id);
      setTerms((prev) => prev.map((t) => t.id === id ? { ...t, suppressed: true } : t));
    } catch (e) {
      toast.error(`Failed to suppress: ${e}`);
    }
  };

  const handleDelete = async (id: number) => {
    try {
      await commands.vocabDeleteTerm(id);
      setTerms((prev) => prev.filter((t) => t.id !== id));
    } catch (e) {
      toast.error(`Failed to delete: ${e}`);
    }
  };

  const handleImport = async () => {
    const lines = importText.split("\n").map((l) => l.trim()).filter(Boolean);
    if (!lines.length) return;
    setImporting(true);
    try {
      const count = await commands.vocabImportTerms(lines);
      toast.success(`Imported ${count} term${count !== 1 ? "s" : ""}`);
      setImportText("");
      setShowImport(false);
      await loadTerms();
    } catch (e) {
      toast.error(`Import failed: ${e}`);
    } finally {
      setImporting(false);
    }
  };

  const filteredTerms = terms.filter((t) =>
    t.term.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <div className="space-y-4">
      <div className="flex gap-1 border-b border-mid-gray/20">
        {(["terms", "corrections"] as Tab[]).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={`px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors capitalize ${
              activeTab === tab
                ? "border-logo-primary text-text"
                : "border-transparent text-mid-gray hover:text-text"
            }`}
          >
            {tab === "terms" ? "Terms" : "Corrections"}
          </button>
        ))}
      </div>

      {activeTab === "terms" && (
        <div className="space-y-3">
          <div className="flex items-center gap-2 flex-wrap">
            <Input
              type="text"
              className="flex-1 min-w-32 max-w-56"
              value={newTerm}
              onChange={(e) => setNewTerm(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleAddTerm()}
              placeholder="New term…"
              variant="compact"
              disabled={addingTerm}
            />
            <Button onClick={handleAddTerm} disabled={!newTerm.trim() || addingTerm} variant="primary" size="md">
              + Add Term
            </Button>
            <Button onClick={() => setShowImport((v) => !v)} variant="secondary" size="md">
              Import
            </Button>
          </div>

          {showImport && (
            <div className="space-y-2">
              <Textarea
                value={importText}
                onChange={(e) => setImportText(e.target.value)}
                placeholder="One term per line…"
                rows={5}
                disabled={importing}
              />
              <div className="flex gap-2">
                <Button onClick={handleImport} disabled={!importText.trim() || importing} variant="primary" size="md">
                  {importing ? "Importing…" : "Import All"}
                </Button>
                <Button onClick={() => { setShowImport(false); setImportText(""); }} variant="secondary" size="md">
                  Cancel
                </Button>
              </div>
            </div>
          )}

          <Input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Filter terms…"
            variant="compact"
          />

          {termsLoading ? (
            <p className="text-sm text-mid-gray px-1">Loading…</p>
          ) : filteredTerms.length === 0 ? (
            <p className="text-sm text-mid-gray px-1">
              {search ? "No matching terms." : "No vocabulary terms yet."}
            </p>
          ) : (
            <SettingsGroup>
              {filteredTerms.map((term) => (
                <TermRow key={term.id} term={term} onSuppress={handleSuppress} onDelete={handleDelete} />
              ))}
            </SettingsGroup>
          )}
        </div>
      )}

      {activeTab === "corrections" && (
        <div className="space-y-3">
          {correctionsLoading ? (
            <p className="text-sm text-mid-gray px-1">Loading…</p>
          ) : corrections.length === 0 ? (
            <p className="text-sm text-mid-gray px-1">
              No corrections recorded yet. They appear here after Whisper auto-learns from repeated patterns.
            </p>
          ) : (
            <SettingsGroup>
              {corrections.map((c, i) => <CorrectionRow key={i} entry={c} />)}
            </SettingsGroup>
          )}
        </div>
      )}
    </div>
  );
});

interface TermRowProps {
  term: VocabTerm;
  onSuppress: (id: number) => void;
  onDelete: (id: number) => void;
}

const TermRow: React.FC<TermRowProps> = React.memo(({ term, onSuppress, onDelete }) => (
  <div className="flex items-center gap-3 px-4 py-2">
    <span className={`flex-1 text-sm font-mono truncate ${term.suppressed ? "line-through text-mid-gray" : ""}`}>
      {term.term}
    </span>
    <span className={`text-xs px-1.5 py-0.5 rounded border ${
      term.source === "manual"
        ? "border-logo-primary/40 text-logo-primary"
        : "border-mid-gray/30 text-mid-gray"
    }`}>
      {term.source === "manual" ? "manual" : "auto"}
    </span>
    <div className="w-12 h-1.5 rounded-full bg-mid-gray/20 overflow-hidden" title={`Weight: ${term.weight}`}>
      <div className="h-full rounded-full bg-logo-primary" style={{ width: `${Math.min(100, (term.weight / 5) * 100)}%` }} />
    </div>
    <Button onClick={() => onSuppress(term.id)} disabled={term.suppressed} variant="ghost" size="sm">
      Suppress
    </Button>
    <Button onClick={() => onDelete(term.id)} variant="danger-ghost" size="sm">
      Delete
    </Button>
  </div>
));

const CorrectionRow: React.FC<{ entry: CorrectionEntry }> = React.memo(({ entry }) => (
  <div className="flex items-center gap-2 px-4 py-2 text-sm">
    <span className="font-mono text-mid-gray">{entry.original}</span>
    <span className="text-mid-gray">→</span>
    <span className="font-mono">{entry.corrected}</span>
    <span className="ml-auto text-xs text-mid-gray">×{entry.times_seen}</span>
  </div>
));
