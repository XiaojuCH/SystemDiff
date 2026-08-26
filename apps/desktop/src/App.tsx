import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { detectLocale, translate } from "./i18n";
import type {
  DesktopChangeGroup,
  DesktopChangeItem,
  DesktopCommandError,
  DesktopField,
  DesktopPresentation,
  DesktopSessionState,
  DesktopValue,
  Locale,
  SessionStage,
} from "./types";

const initialState: DesktopSessionState = {
  stage: "ready",
  presentation: null,
  cleanup_pending: false,
};

function normalizeError(error: unknown): DesktopCommandError {
  if (typeof error === "object" && error !== null) {
    const candidate = error as Partial<DesktopCommandError>;
    if (
      typeof candidate.code === "string" &&
      typeof candidate.message_id === "string" &&
      typeof candidate.technical_details === "string"
    ) {
      return candidate as DesktopCommandError;
    }
  }
  if (typeof error === "string") {
    try {
      return normalizeError(JSON.parse(error));
    } catch {
      return { code: "internal", message_id: "error.internal", technical_details: error };
    }
  }
  return {
    code: "internal",
    message_id: "error.internal",
    technical_details: "The desktop command returned an unrecognized error.",
  };
}

function localTime(locale: Locale, utc: string): string {
  const value = new Date(utc);
  if (Number.isNaN(value.getTime())) return utc;
  return new Intl.DateTimeFormat(locale, {
    hour: "numeric",
    minute: "2-digit",
  }).format(value);
}

function groupHeading(locale: Locale, group: DesktopChangeGroup): string {
  return translate(locale, group.heading_message_id);
}

function displayValue(locale: Locale, value: DesktopValue): string {
  switch (value.kind) {
    case "evidence":
      return value.value;
    case "message":
      return translate(locale, value.message_id);
    case "number":
      return String(value.value);
    case "boolean":
      return translate(locale, value.value ? "value.boolean.true" : "value.boolean.false");
    case "evidence_list":
      return value.values.length === 0 ? translate(locale, "value.none") : value.values.join(" · ");
  }
}

function Field({ field, locale }: { field: DesktopField; locale: Locale }) {
  const label = translate(locale, field.field_id);
  const fieldClass = `field-${field.field_id.replaceAll(".", "-")}`;
  if (field.mode === "changed") {
    return (
      <div className={`evidence-field evidence-comparison ${fieldClass}`}>
        <dt>{label}</dt>
        <dd>
          <span><small>{translate(locale, "field.before")}</small>{displayValue(locale, field.before)}</span>
          <span className="comparison-arrow" aria-hidden="true">→</span>
          <span><small>{translate(locale, "field.after")}</small>{displayValue(locale, field.after)}</span>
        </dd>
      </div>
    );
  }
  return (
    <div className={`evidence-field ${fieldClass}`}>
      <dt>{label}</dt>
      <dd>{displayValue(locale, field.value)}</dd>
    </div>
  );
}

function ChangeCard({ item, locale }: { item: DesktopChangeItem; locale: Locale }) {
  const marker = item.change === "added" ? "+" : item.change === "removed" ? "−" : item.change === "inconclusive" ? "?" : "~";
  return (
    <article className={`change-card change-${item.change}`}>
      <div className="change-marker" aria-hidden="true">{marker}</div>
      <div className="change-body">
        <h3>{displayValue(locale, item.headline)}</h3>
        <p className="change-meaning">{translate(locale, item.message_id)}</p>
        {item.fields.length > 0 && (
          <dl className="evidence-list">
            {item.fields.map((field, index) => <Field key={`${field.field_id}-${index}`} field={field} locale={locale} />)}
          </dl>
        )}
      </div>
    </article>
  );
}

function Results({
  presentation,
  locale,
  onNewCapture,
  working,
  cleanupPending,
}: {
  presentation: DesktopPresentation;
  locale: Locale;
  onNewCapture: () => Promise<void>;
  working: boolean;
  cleanupPending: boolean;
}) {
  const [technicalOpen, setTechnicalOpen] = useState(false);
  const [technical, setTechnical] = useState<string | null>(null);
  const [technicalError, setTechnicalError] = useState<string | null>(null);
  const confirmed = presentation.summary.confirmed_change_count;
  const inconclusive = presentation.summary.inconclusive_change_count;

  const toggleTechnical = async () => {
    const next = !technicalOpen;
    setTechnicalOpen(next);
    if (next && technical === null && technicalError === null) {
      try {
        setTechnical(await invoke<string>("get_technical_details"));
      } catch (error) {
        setTechnicalError(normalizeError(error).technical_details);
      }
    }
  };

  const summary = confirmed === 0
    ? translate(locale, inconclusive > 0 ? "results.noConfirmedChanges" : "results.noChanges")
    : translate(locale, confirmed === 1 ? "results.change.one" : "results.change.many", { count: confirmed });

  return (
    <main className="app-shell results-shell">
      <header className="results-header">
        <div>
          <p className="eyebrow">SystemDiff</p>
          <h1>{summary}</h1>
          {inconclusive > 0 && (
            <p className="inconclusive-summary">
              {translate(locale, inconclusive === 1 ? "results.inconclusive.one" : "results.inconclusive.many", { count: inconclusive })}
            </p>
          )}
        </div>
        <div className="capture-times" aria-label={translate(locale, "results.captureTimes")}>
          <span>{translate(locale, "results.started", { time: localTime(locale, presentation.started_at_utc) })}</span>
          <span>{translate(locale, "results.finished", { time: localTime(locale, presentation.finished_at_utc) })}</span>
        </div>
      </header>

      <div className="results-content">
        {presentation.groups.map((group) => (
          <section className="result-group" key={group.group_id}>
            <h2>{groupHeading(locale, group)}</h2>
            {group.items.length === 0 ? (
              <p className="empty-state">{translate(locale, group.empty_message_id)}</p>
            ) : (
              <div className="change-list">
                {group.items.map((item, index) => <ChangeCard key={`${item.message_id}-${index}`} item={item} locale={locale} />)}
              </div>
            )}
          </section>
        ))}

        {presentation.coverage_notices.length > 0 && (
          <section className="coverage-notice">
            <div className="notice-icon" aria-hidden="true">i</div>
            <div>
              <h2>{translate(locale, "coverage.heading")}</h2>
              {presentation.coverage_notices.map((notice, index) => (
                <p key={`${notice.message_id}-${index}`}>
                  {translate(locale, notice.scope_message_id)}
                </p>
              ))}
            </div>
          </section>
        )}

        {cleanupPending && (
          <section className="cleanup-notice" role="alert">
            <div className="notice-icon" aria-hidden="true">!</div>
            <div>
              <h2>{translate(locale, "cleanup.heading")}</h2>
              <p>{translate(locale, "cleanup.pending")}</p>
            </div>
          </section>
        )}

        <section className="technical-panel">
          <button className="disclosure" type="button" aria-expanded={technicalOpen} onClick={toggleTechnical}>
            <span aria-hidden="true">{technicalOpen ? "▾" : "▸"}</span>
            {translate(locale, technicalOpen ? "technical.hide" : "technical.show")}
          </button>
          {technicalOpen && (
            <pre>{technicalError ?? technical ?? translate(locale, "technical.loading")}</pre>
          )}
        </section>
      </div>

      <footer className="results-actions">
        <button className="primary-button" type="button" disabled={working} onClick={() => void onNewCapture()}>
          {translate(locale, "results.new")}
        </button>
      </footer>
    </main>
  );
}

function Busy({ stage, locale }: { stage: Extract<SessionStage, "starting" | "finishing">; locale: Locale }) {
  return (
    <main className="app-shell centered-shell" aria-busy="true">
      <div className="brand-mark" aria-hidden="true"><span></span><span></span></div>
      <p className="eyebrow">SystemDiff</p>
      <h1>{translate(locale, `busy.${stage}.title`)}</h1>
      <p className="lead">{translate(locale, `busy.${stage}.body`)}</p>
      <div className="indeterminate-track" role="progressbar" aria-label={translate(locale, `busy.${stage}.title`)}>
        <span></span>
      </div>
    </main>
  );
}

function Ready({ locale, onStart, working }: { locale: Locale; onStart: () => Promise<void>; working: boolean }) {
  return (
    <main className="app-shell ready-shell">
      <section className="hero">
        <div className="brand-mark" aria-hidden="true"><span></span><span></span></div>
        <p className="eyebrow">SystemDiff</p>
        <h1>{translate(locale, "app.promise")}</h1>
        <p className="lead">{translate(locale, "ready.body")}</p>
        <button className="primary-button hero-action" type="button" disabled={working} onClick={() => void onStart()}>
          {translate(locale, "ready.start")}
        </button>
      </section>

      <section className="checks-card">
        <h2>{translate(locale, "checks.heading")}</h2>
        <ul>
          <li><span className="check-icon" aria-hidden="true">✓</span><span>{translate(locale, "checks.startup")}</span></li>
          <li><span className="check-icon" aria-hidden="true">✓</span><span>{translate(locale, "checks.services")}</span></li>
          <li className="coming-soon"><span className="check-icon" aria-hidden="true">○</span><span>{translate(locale, "checks.tasks")}</span><small>{translate(locale, "checks.comingSoon")}</small></li>
        </ul>
      </section>

      <footer className="trust-row">
        <span>{translate(locale, "trust.local")}</span><i>·</i>
        <span>{translate(locale, "trust.readOnly")}</span><i>·</i>
        <span>{translate(locale, "trust.telemetry")}</span>
      </footer>
    </main>
  );
}

function Capturing({ locale, onFinish, onCancel, working }: { locale: Locale; onFinish: () => Promise<void>; onCancel: () => Promise<void>; working: boolean }) {
  return (
    <main className="app-shell centered-shell capture-shell">
      <div className="success-ring" aria-hidden="true">✓</div>
      <p className="status-label">{translate(locale, "capturing.done")}</p>
      <h1>{translate(locale, "capturing.title")}</h1>
      <p className="lead">{translate(locale, "capturing.body")}</p>
      <div className="action-row">
        <button className="primary-button" type="button" disabled={working} onClick={() => void onFinish()}>{translate(locale, "capturing.finish")}</button>
        <button className="secondary-button" type="button" disabled={working} onClick={() => void onCancel()}>{translate(locale, "capturing.cancel")}</button>
      </div>
    </main>
  );
}

function ErrorView({ error, locale, onRecover, working }: { error: DesktopCommandError; locale: Locale; onRecover: () => Promise<void>; working: boolean }) {
  const restartRequired = error.code === "another_instance_running"
    || error.code === "bootstrap_storage_failed";
  return (
    <main className="app-shell centered-shell error-shell" role="alert">
      <div className="error-symbol" aria-hidden="true">!</div>
      <p className="eyebrow">SystemDiff</p>
      <h1>{translate(locale, "error.title")}</h1>
      <p className="lead">{translate(locale, error.message_id)}</p>
      <p className="muted-copy">
        {translate(locale, restartRequired ? "error.restart" : "error.body")}
      </p>
      {!restartRequired && (
        <button className="primary-button" type="button" disabled={working} onClick={() => void onRecover()}>{translate(locale, "error.retry")}</button>
      )}
      <details className="error-details">
        <summary>{translate(locale, "error.technical")}</summary>
        <pre>{`${error.code}: ${error.technical_details}`}</pre>
      </details>
    </main>
  );
}

export default function App() {
  const [locale, setLocale] = useState<Locale>(() => detectLocale());
  const [session, setSession] = useState<DesktopSessionState>(initialState);
  const [error, setError] = useState<DesktopCommandError | null>(null);
  const [working, setWorking] = useState(true);
  const inFlight = useRef(false);

  const invokeState = useCallback(async (command: string): Promise<void> => {
    if (inFlight.current) return;
    inFlight.current = true;
    setWorking(true);
    try {
      setError(null);
      if (command === "start_capture") {
        setSession({ stage: "starting", presentation: null, cleanup_pending: false });
      } else if (command === "finish_capture") {
        setSession((current) => ({
          stage: "finishing",
          presentation: current.presentation,
          cleanup_pending: current.cleanup_pending,
        }));
      }
      setSession(await invoke<DesktopSessionState>(command));
    } catch (commandError) {
      setError(normalizeError(commandError));
    } finally {
      inFlight.current = false;
      setWorking(false);
    }
  }, []);

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  useEffect(() => {
    let disposed = false;
    inFlight.current = true;
    void invoke<DesktopSessionState>("get_session_state")
      .then((next) => {
        if (!disposed) setSession(next);
      })
      .catch((commandError: unknown) => {
        if (!disposed) setError(normalizeError(commandError));
      })
      .finally(() => {
        if (!disposed) {
          inFlight.current = false;
          setWorking(false);
        }
      });
    return () => {
      disposed = true;
    };
  }, []);

  let content;
  if (error !== null) {
    content = <ErrorView error={error} locale={locale} working={working} onRecover={() => invokeState("get_session_state")} />;
  } else if (session.stage === "starting" || session.stage === "finishing") {
    content = <Busy stage={session.stage} locale={locale} />;
  } else if (session.stage === "capturing") {
    content = <Capturing locale={locale} working={working} onFinish={() => invokeState("finish_capture")} onCancel={() => invokeState("cancel_capture")} />;
  } else if (session.stage === "results" && session.presentation !== null) {
    content = <Results presentation={session.presentation} locale={locale} working={working} cleanupPending={session.cleanup_pending} onNewCapture={() => invokeState("cancel_capture")} />;
  } else {
    content = <Ready locale={locale} working={working} onStart={() => invokeState("start_capture")} />;
  }

  return (
    <>
      <button
        className="locale-switch"
        type="button"
        aria-label={translate(locale, "language.switch")}
        onClick={() => setLocale((current) => current === "en-US" ? "zh-CN" : "en-US")}
      >
        {locale === "en-US" ? "简体中文" : "English"}
      </button>
      {content}
    </>
  );
}
