# GitHub PR Review Guide

## Prerequisites
- GitHub CLI (`gh`) installed and authenticated
- Access to the repository

## Step-by-Step PR Review Process

### 1. Fetch PR Information
```bash
# View PR details
gh pr view <PR_NUMBER> --json title,body,state,author,files,additions,deletions

# Get list of changed files
gh pr view <PR_NUMBER> --json files --jq '.files[].path'

# View the diff
gh pr diff <PR_NUMBER>
```

### 2. Review Individual Files
```bash
# Get diff for specific files
gh api repos/{owner}/{repo}/pulls/{PR_NUMBER}/files --jq '.[] | select(.filename == "path/to/file") | .patch'

# Or use gh pr diff with specific files
gh pr diff <PR_NUMBER> -- path/to/file
```

### 3. Add Line-Level Comments
```bash
# Add a single comment on a specific line
gh api repos/{owner}/{repo}/pulls/{PR_NUMBER}/comments \
  --method POST \
  --field body='Your comment here' \
  --field commit_id=$(gh pr view <PR_NUMBER> --json headRefOid -q .headRefOid) \
  --field path='path/to/file' \
  --field line=<LINE_NUMBER> \
  --field side='RIGHT'
```

### 4. Create a Full Review
```bash
# Approve the PR
gh pr review <PR_NUMBER> --approve --body "LGTM! Great work."

# Request changes
gh pr review <PR_NUMBER> --request-changes --body "Please address the comments."

# Comment without approval/rejection
gh pr review <PR_NUMBER> --comment --body "I have some suggestions."
```

### 5. Add Multiple Comments in One Review
```bash
# Create a review with multiple line comments (use a JSON file)
cat > review.json << 'EOF'
{
  "event": "COMMENT",
  "body": "Review summary here",
  "comments": [
    {
      "path": "file1.js",
      "line": 10,
      "body": "Comment 1"
    },
    {
      "path": "file2.js", 
      "line": 25,
      "body": "Comment 2"
    }
  ]
}
EOF

gh api repos/{owner}/{repo}/pulls/{PR_NUMBER}/reviews \
  --method POST \
  --input review.json
```

## Common Review Patterns

### 1. Check for Common Issues
- **Invalid Rust Edition**: As of 2025, Rust 2024 is the latest edition (requires Rust 1.85+)
- **CI/CD**: Ensure workflows use latest action versions (e.g., `actions/checkout@v4`)
- **Code Quality**: Check for linting, formatting, and test coverage
- **Security**: Look for hardcoded secrets, vulnerable dependencies
- **Documentation**: Ensure code is properly documented

### 2. Best Practices for Comments
- Be specific about line numbers
- Provide constructive feedback
- Suggest alternatives when pointing out issues
- Use code blocks for suggested changes
- Reference documentation or best practices

### 3. Checking Package Versions
```bash
# For Rust projects
cargo update --dry-run
cargo outdated

# For Node.js projects
npm outdated
npx npm-check-updates

# For Python projects
pip list --outdated
```

## Useful GitHub API Endpoints

```bash
# List all comments on a PR
gh api repos/{owner}/{repo}/pulls/{PR_NUMBER}/comments

# List all reviews on a PR
gh api repos/{owner}/{repo}/pulls/{PR_NUMBER}/reviews

# Get review comments for a specific review
gh api repos/{owner}/{repo}/pulls/{PR_NUMBER}/reviews/{REVIEW_ID}/comments

# Delete a comment (if needed)
gh api repos/{owner}/{repo}/pulls/comments/{COMMENT_ID} --method DELETE
```

## Automation Tips

### Create an Alias for Common Reviews
```bash
# Add to ~/.bashrc or ~/.zshrc
alias pr-review='gh pr review --comment --body'
alias pr-approve='gh pr review --approve --body'
alias pr-changes='gh pr review --request-changes --body'
```

### Script for Batch Comments
```bash
#!/bin/bash
PR_NUMBER=$1
OWNER=$(gh repo view --json owner -q .owner.login)
REPO=$(gh repo view --json name -q .name)
COMMIT_ID=$(gh pr view $PR_NUMBER --json headRefOid -q .headRefOid)

add_comment() {
    local path=$1
    local line=$2
    local comment=$3
    
    gh api repos/$OWNER/$REPO/pulls/$PR_NUMBER/comments \
        --method POST \
        --field body="$comment" \
        --field commit_id=$COMMIT_ID \
        --field path="$path" \
        --field line=$line \
        --field side='RIGHT'
}

# Usage
add_comment "src/main.rs" 10 "Consider using Result<T, E> here"
```

## Notes
- The `side` parameter can be 'LEFT' (for deletions) or 'RIGHT' (for additions)
- Line numbers refer to the position in the diff, not the file
- Comments persist even if the PR is updated (they become "outdated")
- Use `--json` flag with `gh` commands for scripting
- Review comments require the exact commit SHA at the time of commenting