import { describe, expect, it } from "vitest";
import { parse as parseToml } from "smol-toml";
import {
  getChimeraHubTemplate,
  getCodexCustomTemplate,
} from "@/config/codexTemplates";
import {
  extractCodexBaseUrl,
  extractCodexModelName,
} from "@/utils/providerConfigUtils";

describe("Codex custom templates", () => {
  it("does not force Codex Goal mode in the custom provider template", () => {
    const template = getCodexCustomTemplate();
    const parsed = parseToml(template.config) as {
      features?: { goals?: boolean };
      model_providers?: Record<string, unknown>;
    };

    expect(template.auth).toEqual({ OPENAI_API_KEY: "" });
    expect(parsed.features?.goals).toBeUndefined();
    expect(parsed.model_providers?.custom).toBeDefined();
  });

  it("provides the editable ChimeraHub key-first template", () => {
    const template = getChimeraHubTemplate();

    expect(template.name).toBe("ChimeraHub");
    expect(template.websiteUrl).toBe("https://api.chimerahub.org/");
    expect(template.auth).toEqual({ OPENAI_API_KEY: "" });
    expect(extractCodexBaseUrl(template.config)).toBe(
      "https://api.chimerahub.org/v1",
    );
    expect(extractCodexModelName(template.config)).toBe("gpt-5.6-sol");
  });
});
