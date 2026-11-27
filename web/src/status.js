// Lesson state, derived from the progress store. Visited, attempted, and
// completed stay separate:
//   - a lesson's own flags are stored under its id (visited, attempted, and
//     completed for reading-only lessons, set by "Mark complete")
//   - an exercise's completion is stored under the exercise id, when its checks
//     all pass
// A lesson with exercises is complete when every one of them is complete.
//
// This module has no DOM code, so the Node tests can use it directly.

export function exercisesOf(lesson) {
  return lesson.body.filter((block) => block.type === "exercise");
}

// The draft key for the example at body[index]. Keep it in step with lesson.js.
export function examplePlaygroundId(lessonId, index) {
  return `${lessonId}#example-${index}`;
}

// The flags shown for a lesson.
export function lessonFlags(progress, lesson) {
  const own = progress.get(lesson.id);
  const exercises = exercisesOf(lesson);
  const exercisesDone =
    exercises.length > 0 && exercises.every((exercise) => progress.get(exercise.id).completed);
  return {
    visited: own.visited,
    attempted: own.attempted,
    completed: own.completed || exercisesDone,
  };
}

// Clears the lesson's flags, each exercise's completion, and the saved drafts
// of its playgrounds, so the lesson starts again from the beginning.
export function resetLessonProgress(progress, lesson) {
  progress.clearLesson(lesson.id);
  lesson.body.forEach((block, index) => {
    if (block.type === "exercise") {
      progress.clearLesson(block.id);
      progress.removeDraft(block.id);
    } else if (block.type === "example") {
      progress.removeDraft(examplePlaygroundId(lesson.id, index));
    }
  });
}
