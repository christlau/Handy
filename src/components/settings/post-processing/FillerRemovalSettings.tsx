import React from "react";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

export const FillerRemovalSettings: React.FC = React.memo(() => {
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const fillerEnabled = getSetting("remove_fillers_enabled") ?? false;
  const falseStartEnabled = getSetting("remove_false_starts_enabled") ?? false;

  return (
    <>
      <ToggleSwitch
        checked={fillerEnabled}
        onChange={(v) => updateSetting("remove_fillers_enabled", v)}
        isUpdating={isUpdating("remove_fillers_enabled")}
        label="Remove filler words"
        description='Removes "um", "uh", "hmm" and similar sounds from transcriptions'
        descriptionMode="tooltip"
        grouped={true}
      />
      {fillerEnabled && (
        <ToggleSwitch
          checked={falseStartEnabled}
          onChange={(v) => updateSetting("remove_false_starts_enabled", v)}
          isUpdating={isUpdating("remove_false_starts_enabled")}
          label="Remove false starts"
          description='Detects "scratch that", "I mean", em-dash breaks and removes the abandoned attempt'
          descriptionMode="tooltip"
          grouped={true}
        />
      )}
    </>
  );
});

FillerRemovalSettings.displayName = "FillerRemovalSettings";
