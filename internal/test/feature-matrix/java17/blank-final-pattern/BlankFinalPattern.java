/**
 * Blank finals assigned exactly once on every branch, including branches that
 * are selected by a pattern variable.
 *
 * The pattern variable is a synthetic symbol created during attribution. If it
 * does not get a local variable slot of its own, its initialization state
 * aliases the first field's slot and the assignments below look like repeats.
 */
class BlankFinalPattern {

    private final Object fromIf;
    private final Object fromConditional;
    private Object mutable;

    BlankFinalPattern(Object node) {
        if (node instanceof String text) {
            this.fromIf = text;
        } else {
            this.fromIf = node;
        }
        this.fromConditional = null;
        this.mutable = node;
    }

    BlankFinalPattern(boolean flag, Object node) {
        this.fromIf = null;
        this.fromConditional = flag ? node : null;
        this.mutable = node;
    }

    Object fromIf() {
        return this.fromIf;
    }

    Object fromConditional() {
        return this.fromConditional;
    }

    Object mutable() {
        return this.mutable;
    }

    /**
     * A guarded pattern: the variable is definitely assigned in the then
     * branch and definitely unassigned in the else branch.
     */
    static String describe(Object node) {
        if (node instanceof Integer number && number > 0) {
            return "positive " + number;
        }
        return "other";
    }

    /**
     * Nested patterns with a blank final local: each path assigns it once.
     */
    static Object pick(Object node) {
        final Object result;
        if (node instanceof String text) {
            if (text.isEmpty()) {
                result = "";
            } else {
                result = text;
            }
        } else {
            result = node;
        }
        return result;
    }
}
