import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { ShowOverlay } from "../ShowOverlay";
import { ModelUnloadTimeoutSetting } from "../ModelUnloadTimeout";
import { CustomWords } from "../CustomWords";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { StartHidden } from "../StartHidden";
import { AutostartToggle } from "../AutostartToggle";
import { ShowTrayIcon } from "../ShowTrayIcon";
import { PasteMethodSetting } from "../PasteMethod";
import { TypingToolSetting } from "../TypingTool";
import { ClipboardHandlingSetting } from "../ClipboardHandling";
import { AutoSubmit } from "../AutoSubmit";
import { PostProcessingToggle } from "../PostProcessingToggle";
import { AppendTrailingSpace } from "../AppendTrailingSpace";
import { HistoryLimit } from "../HistoryLimit";
import { RecordingRetentionPeriodSelector } from "../RecordingRetentionPeriod";
import { ExperimentalToggle } from "../ExperimentalToggle";
import { useSettings } from "../../../hooks/useSettings";
import { KeyboardImplementationSelector } from "../debug/KeyboardImplementationSelector";
import { VoiceActivityDetection } from "../VoiceActivityDetection";
import { AccelerationSelector } from "../AccelerationSelector";
import { LazyStreamClose } from "../LazyStreamClose";
import { Input } from "../../ui/Input";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";

// Inline vocab term type — does not depend on bindings.ts
interface VocabTerm { id: number; term: string; source: string; weight: number; suppressed: boolean; }

const VocabTermsInline: React.FC = React.memo(() => {
  const [terms, setTerms] = useState<VocabTerm[]>([]);
  const [newTerm, setNewTerm] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const result = await invoke<VocabTerm[]>("vocab_list_terms");
      setTerms(result ?? []);
    } catch (e) {
      // Command not yet registered (bindings not regenerated) — silent fail
      setError(String(e));
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const add = async () => {
    const trimmed = newTerm.trim();
    if (trimmed.length < 2) return;
    setLoading(true);
    try {
      const result = await invoke<VocabTerm>("vocab_add_term", { term: trimmed });
      setTerms(prev => [result, ...prev]);
      setNewTerm("");
    } catch (e) { console.error("vocab_add_term:", e); }
    finally { setLoading(false); }
  };

  const remove = async (id: number) => {
    try {
      await invoke("vocab_delete_term", { id });
      setTerms(prev => prev.filter(t => t.id !== id));
    } catch (e) { console.error("vocab_delete_term:", e); }
  };

  return (
    <SettingContainer
      title="Learned Vocabulary"
      description="Terms auto-learned from your edits or added manually. Injected into Whisper's prompt to improve recognition of names and technical terms."
      descriptionMode="tooltip"
      grouped
      layout="stacked"
    >
      <div className="space-y-2">
        <div className="flex gap-2">
          <Input
            type="text"
            value={newTerm}
            onChange={e => setNewTerm(e.target.value)}
            onKeyDown={e => e.key === "Enter" && add()}
            placeholder="Add term…"
            variant="compact"
            disabled={loading}
          />
          <Button onClick={add} disabled={newTerm.trim().length < 2 || loading} variant="primary" size="md">
            + Add
          </Button>
        </div>
        {error ? (
          <p className="text-xs text-mid-gray/50">Vocabulary requires a restart to activate.</p>
        ) : terms.filter(t => !t.suppressed).length > 0 ? (
          <div className="flex flex-wrap gap-1 pt-1">
            {terms.filter(t => !t.suppressed).map(t => (
              <Button key={t.id} onClick={() => remove(t.id)} variant="secondary" size="sm"
                className="inline-flex items-center gap-1">
                <span className="font-mono text-xs">{t.term}</span>
                {t.source !== "manual" && <span className="text-xs opacity-50 ml-1">auto</span>}
                <svg className="w-3 h-3 ml-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </Button>
            ))}
          </div>
        ) : (
          <p className="text-xs text-mid-gray/70">No terms yet. Add manually or they appear automatically after Whisper learns from your repeated corrections.</p>
        )}
      </div>
    </SettingContainer>
  );
});

export const AdvancedSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const experimentalEnabled = getSetting("experimental_enabled") || false;

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.advanced.groups.app")}>
        <StartHidden descriptionMode="tooltip" grouped={true} />
        <AutostartToggle descriptionMode="tooltip" grouped={true} />
        <ShowTrayIcon descriptionMode="tooltip" grouped={true} />
        <ShowOverlay descriptionMode="tooltip" grouped={true} />
        <ModelUnloadTimeoutSetting descriptionMode="tooltip" grouped={true} />
        <ExperimentalToggle descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <SettingsGroup title={t("settings.advanced.groups.output")}>
        <PasteMethodSetting descriptionMode="tooltip" grouped={true} />
        <TypingToolSetting descriptionMode="tooltip" grouped={true} />
        <ClipboardHandlingSetting descriptionMode="tooltip" grouped={true} />
        <AutoSubmit descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <SettingsGroup title={t("settings.advanced.groups.transcription")}>
        <VoiceActivityDetection descriptionMode="tooltip" grouped={true} />
        <CustomWords descriptionMode="tooltip" grouped />
        <VocabTermsInline />
        <AppendTrailingSpace descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <SettingsGroup title={t("settings.advanced.groups.history")}>
        <HistoryLimit descriptionMode="tooltip" grouped={true} />
        <RecordingRetentionPeriodSelector
          descriptionMode="tooltip"
          grouped={true}
        />
      </SettingsGroup>

      {experimentalEnabled && (
        <SettingsGroup title={t("settings.advanced.groups.experimental")}>
          <PostProcessingToggle descriptionMode="tooltip" grouped={true} />
          <KeyboardImplementationSelector
            descriptionMode="tooltip"
            grouped={true}
          />
          <AccelerationSelector descriptionMode="tooltip" grouped={true} />
          <LazyStreamClose descriptionMode="tooltip" grouped={true} />
        </SettingsGroup>
      )}
    </div>
  );
};
